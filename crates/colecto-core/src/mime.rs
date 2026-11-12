//! MIME reading and outgoing attachment helpers. All message content is untrusted.
use crate::Error;
use base64::Engine;
use colecto_extension_api::{AttachmentInfo, MessageContent, OutgoingAttachment};
use mailparse::{DispositionType, MailHeaderMap, ParsedMail};

/// Largest raw message the client opens on demand.
pub(crate) const MAX_RAW: usize = 30_000_000;
/// Largest total attachment size accepted for one outgoing message.
pub const MAX_ATTACHMENTS: usize = 25_000_000;
const MAX_HTML: usize = 2_000_000;

pub(crate) struct Attachment {
    pub name: String,
    pub content_type: String,
    pub data: Vec<u8>,
}

fn undecodable() -> Error {
    Error::Transport("The message could not be decoded.".into())
}

// Mirror mailparse's line-prefix boundary matching before entering its unbounded parser.
fn validate_structure(raw: &[u8]) -> Result<(), Error> {
    fn scan(raw: &[u8], depth: usize, count: &mut usize) -> Result<(), Error> {
        *count += 1;
        if depth > 32 || *count > 1_000 {
            return Err(undecodable());
        }
        let (headers, body) = mailparse::parse_headers(raw).map_err(|_| undecodable())?;
        let ctype = mailparse::parse_content_type(
            &headers.get_first_value("Content-Type").unwrap_or_default(),
        );
        if !ctype.mimetype.starts_with("multipart/") {
            return Ok(());
        }
        let Some(boundary) = ctype.params.get("boundary") else {
            return Ok(());
        };
        if boundary.contains(['\r', '\n']) {
            return Err(undecodable());
        }
        let boundary = format!("--{boundary}");
        let mut starts = Vec::new();
        let mut at = body;
        for line in raw[body..].split_inclusive(|b| *b == b'\n') {
            if raw[at..].starts_with(boundary.as_bytes()) {
                starts.push((at, at + boundary.len()));
            }
            at += line.len();
        }
        if let Some(&(_, mut end)) = starts.first() {
            for next in starts.iter().skip(1).map(Some).chain(std::iter::once(None)) {
                let Some(start) = raw[end..]
                    .iter()
                    .position(|b| *b == b'\n')
                    .map(|i| end + i + 1)
                else {
                    break;
                };
                let part_end = next.map_or(raw.len(), |(at, _)| *at);
                scan(&raw[start..part_end], depth + 1, count)?;
                let Some(&(_, boundary_end)) = next else {
                    break;
                };
                end = boundary_end;
                if raw.get(end..end + 2).is_none_or(|v| v == b"--") {
                    break;
                }
            }
        }
        Ok(())
    }
    scan(raw, 0, &mut 0)
}

pub(crate) fn parse(raw: &[u8]) -> Result<ParsedMail<'_>, Error> {
    validate_structure(raw)?;
    mailparse::parse_mail(raw).map_err(|_| undecodable())
}

fn leaves<'a>(mail: &'a ParsedMail<'a>, out: &mut Vec<&'a ParsedMail<'a>>) {
    let mut pending = vec![mail];
    while let Some(part) = pending.pop() {
        if part.subparts.is_empty() {
            out.push(part);
        }
        pending.extend(part.subparts.iter().rev());
    }
}

fn is_attachment(part: &ParsedMail<'_>) -> bool {
    let disposition = part.get_content_disposition().disposition;
    disposition == DispositionType::Attachment
        || (!matches!(part.ctype.mimetype.as_str(), "text/plain" | "text/html")
            // Inline parts with a Content-ID are images referenced by the HTML body.
            && part.headers.get_first_value("Content-ID").is_none())
}

fn attachment_parts<'a>(mail: &'a ParsedMail<'a>) -> Vec<&'a ParsedMail<'a>> {
    let mut parts = Vec::new();
    leaves(mail, &mut parts);
    parts.retain(|part| is_attachment(part));
    parts
}

fn safe_name(name: &str, index: usize) -> String {
    let name: String = name
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '/' | '\\'))
        .take(200)
        .collect();
    let name = name.trim().trim_start_matches('.');
    if name.is_empty() {
        format!("attachment-{}", index + 1)
    } else {
        name.to_owned()
    }
}

fn info(part: &ParsedMail<'_>, index: usize, size: usize) -> AttachmentInfo {
    let disposition = part.get_content_disposition();
    let name = disposition
        .params
        .get("filename")
        .or_else(|| part.ctype.params.get("name"))
        .map(String::as_str)
        .unwrap_or_default();
    AttachmentInfo {
        index,
        name: safe_name(name, index),
        content_type: part.ctype.mimetype.clone(),
        size,
    }
}

fn first_body(mail: &ParsedMail<'_>, mimetype: &str) -> Result<Option<String>, Error> {
    let mut parts = Vec::new();
    leaves(mail, &mut parts);
    for part in parts {
        if part.ctype.mimetype == mimetype
            && part.get_content_disposition().disposition != DispositionType::Attachment
        {
            let body = part.get_body().map_err(|_| undecodable())?;
            if !body.trim().is_empty() {
                return Ok(Some(body));
            }
        }
    }
    Ok(None)
}

/// Plain text for lists and the reader, derived from HTML when no text part exists.
pub(crate) fn text(mail: &ParsedMail<'_>) -> Result<String, Error> {
    Ok(match first_body(mail, "text/plain")? {
        Some(text) => text,
        None => first_body(mail, "text/html")?
            .map(|html| html_text(&html))
            .unwrap_or_default(),
    })
}

fn truncate(mut text: String, limit: usize) -> String {
    if text.len() > limit {
        let mut end = limit;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
    text
}

pub(crate) fn content(raw: &[u8]) -> Result<MessageContent, Error> {
    let mail = parse(raw)?;
    let header = |name: &str| mail.headers.get_first_value(name).unwrap_or_default();
    let attachments = attachment_parts(&mail)
        .into_iter()
        .enumerate()
        .map(|(index, part)| {
            let size = part.get_body_raw().map_err(|_| undecodable())?.len();
            Ok(info(part, index, size))
        })
        .collect::<Result<_, Error>>()?;
    Ok(MessageContent {
        text: crate::bounded_body(text(&mail)?),
        html: first_body(&mail, "text/html")?.map(|html| truncate(html, MAX_HTML)),
        attachments,
        message_id: header("Message-ID"),
        references: header("References"),
        reply_to: header("Reply-To"),
        to: header("To"),
        cc: header("Cc"),
    })
}

fn decoded(part: &ParsedMail<'_>, index: usize) -> Result<Attachment, Error> {
    let data = part.get_body_raw().map_err(|_| undecodable())?;
    let info = info(part, index, data.len());
    Ok(Attachment {
        name: info.name,
        content_type: info.content_type,
        data,
    })
}

pub(crate) fn attachment(raw: &[u8], index: usize) -> Result<Attachment, Error> {
    let mail = parse(raw)?;
    let parts = attachment_parts(&mail);
    decoded(parts.get(index).ok_or(Error::Missing)?, index)
}

pub(crate) fn attachments(raw: &[u8]) -> Result<Vec<Attachment>, Error> {
    let mail = parse(raw)?;
    attachment_parts(&mail)
        .into_iter()
        .enumerate()
        .map(|(index, part)| decoded(part, index))
        .collect()
}

pub(crate) fn uploads(files: &[OutgoingAttachment]) -> Result<Vec<Attachment>, Error> {
    files
        .iter()
        .enumerate()
        .map(|(index, file)| {
            let data = base64::engine::general_purpose::STANDARD
                .decode(&file.data)
                .map_err(|_| Error::Invalid("Attachment data must be base64".into()))?;
            Ok(Attachment {
                name: safe_name(&file.name, index),
                content_type: file.content_type.clone(),
                data,
            })
        })
        .collect()
}

/// Crude text rendering of HTML for previews; the reader shows the real HTML.
pub(crate) fn html_text(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let mut text = String::new();
    let mut at = 0;
    while let Some(start) = lower[at..].find('<').map(|offset| at + offset) {
        text.push_str(&html[at..start]);
        let tag = &lower[start..];
        let close = if tag.starts_with("<style") {
            "</style>"
        } else if tag.starts_with("<script") {
            "</script>"
        } else if tag.starts_with("<!--") {
            "-->"
        } else {
            ">"
        };
        let name = tag[1..]
            .trim_start_matches('/')
            .split(|c: char| !c.is_ascii_alphanumeric())
            .next()
            .unwrap_or_default();
        if matches!(
            name,
            "br" | "p" | "div" | "tr" | "li" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
        ) {
            text.push('\n');
        }
        match tag.find(close) {
            Some(end) => at = start + end + close.len(),
            None => {
                at = html.len();
                break;
            }
        }
    }
    text.push_str(&html[at..]);
    let text = text
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&");
    let mut out = String::new();
    let mut blank = 0;
    for line in text.lines() {
        let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if line.is_empty() {
            blank += 1;
            continue;
        }
        if !out.is_empty() {
            out.push_str(if blank > 0 { "\n\n" } else { "\n" });
        }
        blank = 0;
        out.push_str(&line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    const RAW: &[u8] = b"From: a@example.test\r\nTo: Me <me@example.test>, b@example.test\r\nCc: c@example.test\r\nMessage-ID: <m1@example.test>\r\nReferences: <m0@example.test>\r\nContent-Type: multipart/mixed; boundary=b\r\n\r\n--b\r\nContent-Type: multipart/related; boundary=r\r\n\r\n--r\r\nContent-Type: text/html\r\n\r\n<style>p{color:red}</style><p>Hi&nbsp;<b>there</b></p><img src=\"cid:logo\">\r\n--r\r\nContent-Type: image/png\r\nContent-ID: <logo>\r\n\r\nPNG\r\n--r--\r\n--b\r\nContent-Type: application/pdf; name=\"../report.pdf\"\r\nContent-Disposition: attachment\r\nContent-Transfer-Encoding: base64\r\n\r\nUERG\r\n--b--\r\n";

    #[test]
    fn rejects_hostile_nesting_and_part_counts_before_parsing() {
        let mut raw = String::new();
        for i in 0..5_000 {
            raw.push_str(&format!(
                "Content-Type: multipart/mixed; boundary=b{i}\r\n\r\n--b{i}\r\n"
            ));
        }
        raw.push_str("\r\nbody");
        assert!(validate_structure(raw.as_bytes()).is_err());
        let wide = format!(
            "Content-Type: multipart/mixed; boundary=b\r\n\r\n{}--b--\r\n",
            "--b\r\n\r\nx\r\n".repeat(1_001)
        );
        assert!(validate_structure(wide.as_bytes()).is_err());
        assert!(parse(wide.as_bytes()).is_err());
    }

    #[test]
    fn reads_html_threading_and_attachments_but_not_inline_images() {
        let content = content(RAW).unwrap();
        assert_eq!(content.text, "Hi there");
        assert!(content.html.unwrap().contains("<b>there</b>"));
        assert_eq!(content.message_id, "<m1@example.test>");
        assert_eq!(content.references, "<m0@example.test>");
        assert_eq!(content.cc, "c@example.test");
        assert_eq!(
            content.attachments,
            vec![AttachmentInfo {
                index: 0,
                name: "report.pdf".into(),
                content_type: "application/pdf".into(),
                size: 3
            }]
        );
        assert_eq!(attachment(RAW, 0).unwrap().data, b"PDF");
        assert!(matches!(attachment(RAW, 1), Err(Error::Missing)));
    }

    #[test]
    fn html_text_drops_markup_scripts_and_styles() {
        assert_eq!(
            html_text("<script>x<y</script><div>One &amp; two</div><p>Three</p><!-- <p>no</p> -->"),
            "One & two\n\nThree"
        );
    }
}

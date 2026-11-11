//! MCP adapters use the same authenticated mail services as HTTP callers.
use crate::{Host, api_keys::Principal};
use axum::http::HeaderMap;
use rmcp::{
    ErrorData, ServerHandler,
    model::*,
    service::{RequestContext, RoleServer},
    transport::{
        StreamableHttpServerConfig, StreamableHttpService,
        streamable_http_server::session::local::LocalSessionManager,
    },
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{borrow::Cow, sync::Arc};

#[derive(Clone)]
pub(crate) struct MailMcp {
    host: Host,
}
pub(crate) fn http(host: Host) -> StreamableHttpService<MailMcp, LocalSessionManager> {
    let mut config = StreamableHttpServerConfig::default()
        .with_json_response(true)
        .with_max_request_body_bytes(512_000);
    config.legacy_session_mode = false;
    StreamableHttpService::new(
        move || Ok(MailMcp { host: host.clone() }),
        Arc::default(),
        config,
    )
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    account: String,
    id: Option<String>,
    ids: Option<Vec<String>>,
    seen: Option<bool>,
    flagged: Option<bool>,
    draft: Option<colecto_extension_api::DraftInput>,
    label: Option<colecto_extension_api::LabelInput>,
    folder: Option<String>,
    #[serde(default)]
    q: String,
    target: Option<String>,
}
const TOOLS: &[&str] = &[
    "messages.flags",
    "messages.move",
    "messages.archive",
    "messages.delete",
    "account.read",
    "messages.list",
    "folders.list",
    "messages.cached",
    "messages.refresh",
    "messages.read",
    "drafts.list",
    "drafts.read",
    "drafts.write",
    "messages.label",
    "labels.list",
    "messages.send",
    "send.status",
    "app.navigate",
];
fn tool(name: &str) -> Tool {
    let description = if name == "messages.send" {
        "Request human approval to deliver an immutable message. Returns pending ID immediately. Never sends directly; only the owner can approve in Colecto."
    } else if name == "send.status" {
        "Read your own delivery request status. Poll at most once per second. Uncertain means do not retry automatically."
    } else if name == "messages.cached" {
        "Read the last successful local folder snapshot immediately, without provider access. Search is unsupported. Includes fetched_at; null means freshness is unknown."
    } else if matches!(name, "messages.list" | "messages.refresh") {
        "List or search Inbox or Sent. Set q to search the provider by sender, subject and text; an empty q lists the folder and updates the bounded local cache. Requires messages.list; full bodies also require messages.read."
    } else if name == "drafts.list" {
        "List this account's saved drafts. Filter them yourself by recipient, subject or body."
    } else if name == "app.navigate" {
        "Open a page in the owner's running Colecto windows, such as a folder, the composer or a Settings page. Mail folders and compose switch those windows to this account. Requires app.navigate."
    } else {
        "Access this account using the authenticated key's current grants."
    };
    let mut required = vec!["account"];
    match name {
        "messages.read" | "drafts.read" | "send.status" => required.push("id"),
        "drafts.write" | "messages.send" => required.push("draft"),
        "app.navigate" => required.push("target"),
        "messages.label" => required.push("label"),
        "messages.flags" | "messages.archive" | "messages.delete" => required.push("ids"),
        "messages.move" => {
            required.push("ids");
            required.push("folder");
        }
        _ => {}
    }
    let schema = json!({
        "type":"object", "additionalProperties":false, "required":required,
        "properties":{
            "account":{"type":"string","minLength":1,"maxLength":128},
            "id":{"type":"string","minLength":1,"maxLength":256},
            "folder":{"type":"string"},
            "q":{"type":"string","maxLength":1024},
            "ids":{"type":"array","minItems":1,"maxItems":500,"items":{"type":"string"}},
            "seen":{"type":"boolean"},
            "flagged":{"type":"boolean"},
            "target":{"type":"string","enum":colecto_extension_api::NAVIGATION_TARGETS},
            "draft":{
                "type":"object", "additionalProperties":false,
                "required":["recipient","subject","body"],
                "properties":{
                    "id":{"type":["string","null"],"maxLength":256},
                    "recipient":{"type":"string","maxLength":320},
                    "subject":{"type":"string","maxLength":998},
                    "body":{"type":"string","maxLength":100000}
                }
            },
            "label":{
                "type":"object", "additionalProperties":false, "required":["name"],
                "properties":{
                    "name":{"type":"string","maxLength":80},
                    "message_id":{"type":["string","null"]},
                    "remove":{"type":"boolean","default":false}
                }
            }
        }
    });
    Tool::new(
        name.to_owned(),
        description,
        Arc::new(schema.as_object().unwrap().clone()),
    )
}
impl MailMcp {
    async fn execute(
        &self,
        principal: Principal,
        name: &str,
        input: Input,
    ) -> Result<Value, crate::Failure> {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-colecto-account",
            input.account.parse().map_err(|_| crate::forbidden())?,
        );
        let state = axum::extract::State(self.host.clone());
        let identity = axum::Extension(principal.clone());
        let path = axum::extract::Path(input.id.unwrap_or_default());
        macro_rules! value {
            ($call:expr) => {
                $call.await.and_then(|axum::Json(value)| {
                    serde_json::to_value(value).map_err(|_| {
                        crate::approval_failure("Cannot serialize mail response".into())
                    })
                })
            };
        }
        match name {
            "messages.flags" | "messages.move" | "messages.archive" | "messages.delete" => {
                value!(crate::organize_messages(
                    state,
                    identity,
                    headers,
                    axum::extract::Path(name.trim_start_matches("messages.").to_owned()),
                    Ok(axum::Json(crate::OrganizeInput {
                        ids: input.ids.unwrap_or_default(),
                        seen: input.seen,
                        flagged: input.flagged,
                        folder: input.folder
                    }))
                ))
            }
            "account.read" => value!(crate::account(state, identity, headers)),
            "folders.list" => value!(crate::folders(state, identity, headers)),
            "messages.list" | "messages.refresh" => value!(crate::messages(
                state,
                identity,
                headers,
                axum::extract::Query(crate::MessageQuery {
                    folder: input.folder.unwrap_or_else(crate::inbox_folder),
                    q: input.q
                })
            )),
            "messages.cached" => value!(crate::cached_messages(
                state,
                identity,
                headers,
                axum::extract::Query(crate::MessageQuery {
                    folder: input.folder.unwrap_or_else(crate::inbox_folder),
                    q: input.q
                })
            )),
            "messages.read" => value!(crate::message(state, identity, headers, path)),
            "drafts.list" => value!(crate::drafts(state, identity, headers)),
            "drafts.read" => value!(crate::draft(state, identity, headers, path)),
            "drafts.write" => {
                let draft = input
                    .draft
                    .ok_or_else(|| crate::approval_failure("Draft required".into()))?;
                value!(crate::save_draft(
                    state,
                    identity,
                    headers,
                    Ok(axum::Json(draft))
                ))
            }
            "labels.list" => value!(crate::labels(state, identity, headers)),
            "messages.label" => {
                let label = input
                    .label
                    .ok_or_else(|| crate::approval_failure("Label required".into()))?;
                value!(crate::update_label(
                    state,
                    identity,
                    headers,
                    Ok(axum::Json(label))
                ))
            }
            "messages.send" => {
                let draft = input
                    .draft
                    .ok_or_else(|| crate::approval_failure("Draft required".into()))?;
                value!(crate::send(state, identity, headers, Ok(axum::Json(draft))))
            }
            "app.navigate" => {
                let target = input
                    .target
                    .filter(|t| colecto_extension_api::NAVIGATION_TARGETS.contains(&t.as_str()))
                    .ok_or_else(|| crate::approval_failure("Unknown navigation target".into()))?;
                crate::navigate(&self.host, &principal, &input.account, &target)
            }
            "send.status" => {
                crate::request_status(&self.host, &principal, &path.0, Some(&input.account)).await
            }
            _ => Err(crate::approval_failure("Unknown mail tool".into())),
        }
    }
}
impl ServerHandler for MailMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
    }
    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Borrowed(&[ProtocolVersion::V_2025_11_25])
    }
    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult {
            tools: TOOLS.iter().map(|name| tool(name)).collect(),
            ..Default::default()
        })
    }
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        if !TOOLS.contains(&request.name.as_ref()) {
            return Err(ErrorData::invalid_params("Unknown mail tool", None));
        }
        let principal = context
            .extensions
            .get::<axum::http::request::Parts>()
            .and_then(|parts| parts.extensions.get::<Principal>())
            .cloned();
        let result = async {
            let principal = principal
                .filter(|p| matches!(p, Principal::ApiKey(_)))
                .ok_or_else(crate::forbidden)?;
            let permit = self
                .host
                .execution
                .clone()
                .try_acquire_owned()
                .map_err(|_| {
                    crate::Failure(
                        axum::http::StatusCode::TOO_MANY_REQUESTS,
                        "Mail service busy; try again".into(),
                    )
                })?;
            let input: Input =
                serde_json::from_value(Value::Object(request.arguments.unwrap_or_default()))
                    .map_err(|_| crate::approval_failure("Invalid mail tool input".into()))?;
            if input.account.is_empty()
                || input.account.len() > 128
                || input.id.as_ref().is_some_and(|id| id.len() > 256)
            {
                return Err(crate::approval_failure("Invalid mail tool input".into()));
            }
            if matches!(
                request.name.as_ref(),
                "messages.read" | "drafts.read" | "send.status"
            ) && input.id.as_ref().is_none_or(String::is_empty)
            {
                return Err(crate::approval_failure(
                    "Message or request ID required".into(),
                ));
            }
            let service = self.clone();
            // Keep execution bounded even when the client disconnects during provider work.
            tokio::spawn(async move {
                let _permit = permit;
                service.execute(principal, &request.name, input).await
            })
            .await
            .map_err(|_| crate::approval_failure("Mail service unavailable".into()))?
        }
        .await;
        Ok(match result {
            Ok(value) => CallToolResult::structured(value),
            Err(error) => {
                let value =
                    json!({"category":error.category(),"error":error.1,"status":error.0.as_u16()});
                CallToolResult::structured_error(value)
            }
        }
        .into())
    }
}

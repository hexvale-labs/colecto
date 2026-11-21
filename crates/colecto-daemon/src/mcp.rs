//! MCP adapters use the same authenticated `MailService` operations as HTTP callers.
use crate::Failure;
use colecto_core::{DraftInput, LabelInput, MAX_LIST_OFFSET, NAVIGATION_TARGETS};
use colecto_service::{
    MailService, MessageQuery, OrganizeInput, api_keys::Principal, error::ServiceError,
    settings::SettingsInput,
};
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
    service: MailService,
}

pub(crate) fn http(
    service: MailService,
    authority: String,
) -> StreamableHttpService<MailMcp, LocalSessionManager> {
    let mut config = StreamableHttpServerConfig::default()
        .with_json_response(true)
        .with_max_request_body_bytes(512_000)
        .with_allowed_hosts([authority]);
    config.legacy_session_mode = false;
    StreamableHttpService::new(
        move || {
            Ok(MailMcp {
                service: service.clone(),
            })
        },
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
    draft: Option<DraftInput>,
    label: Option<LabelInput>,
    folder: Option<String>,
    #[serde(default)]
    q: String,
    #[serde(default)]
    offset: usize,
    target: Option<String>,
    settings: Option<SettingsInput>,
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
    "settings.read",
    "settings.update",
];

fn tool(name: &str) -> Tool {
    let description = if name == "messages.send" {
        "Request human approval to deliver an immutable message. Returns pending ID immediately. Never sends directly; only the owner can approve in Colecto."
    } else if name == "send.status" {
        "Read your own delivery request status. Poll at most once per second. Uncertain means do not retry automatically."
    } else if name == "messages.cached" {
        "Read the last successful local folder snapshot immediately, without provider access. Search is unsupported. Includes fetched_at; null means freshness is unknown."
    } else if matches!(name, "messages.list" | "messages.refresh") {
        "List or search a folder: inbox, sent, or a folder id from folders.list. Set q to search the provider by sender, subject and text; an empty q lists the newest 100 messages and, for Inbox and Sent, updates the bounded local cache. Set offset to skip that many of the newest messages and read an older page. Requires messages.list; full bodies also require messages.read."
    } else if name == "drafts.list" {
        "List this account's saved drafts. Filter them yourself by recipient, subject or body."
    } else if name == "app.navigate" {
        "Open a page in the owner's running Colecto windows, such as a folder, the composer or a Settings page. Mail folders and compose switch those windows to this account. Requires app.navigate."
    } else if name == "settings.read" {
        "Read the settings this key may change, by area: appearance (needs settings.appearance), notifications (settings.notifications), and mail (settings.mail). Credentials, connections, background services, API keys and accounts are never included."
    } else if name == "settings.update" {
        "Change settings in the areas this key was granted. Fields you leave out stay as they are. Values are checked like Settings checks them, nothing is saved if any part is refused, and open Colecto windows update at once. Returns the settings as settings.read does."
    } else {
        "Access this account using the authenticated key's current grants."
    };
    let mut required = vec!["account"];
    match name {
        "messages.read" | "drafts.read" | "send.status" => required.push("id"),
        "drafts.write" | "messages.send" => required.push("draft"),
        "app.navigate" => required.push("target"),
        "settings.update" => required.push("settings"),
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
            "offset":{"type":"integer","minimum":0,"maximum":MAX_LIST_OFFSET},
            "ids":{"type":"array","minItems":1,"maxItems":500,"items":{"type":"string"}},
            "seen":{"type":"boolean"},
            "flagged":{"type":"boolean"},
            "target":{"type":"string","enum":NAVIGATION_TARGETS},
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
            "settings":{
                "type":"object", "additionalProperties":false,
                "properties":{
                    "appearance":{
                        "type":"object", "additionalProperties":false,
                        "properties":{
                            "accent":{"type":"string","description":"#rrggbb, or empty for the default mint"},
                            "font":{"type":"string","maxLength":64,"description":"A bundled font (mona, inter, geist, plex, atkinson) or an installed system family"},
                            "font_size":{"type":"integer","minimum":13,"maximum":18},
                            "text_spacing":{
                                "type":"object", "additionalProperties":false,
                                "properties":{
                                    "letter":{"type":"integer","minimum":0,"maximum":2},
                                    "word":{"type":"integer","minimum":0,"maximum":2},
                                    "line":{"type":"integer","minimum":0,"maximum":2},
                                    "paragraph":{"type":"integer","minimum":0,"maximum":2}
                                }
                            },
                            "list_density":{"type":"string","enum":["full","medium","compact"]},
                            "date_groups":{"type":"boolean"}
                        }
                    },
                    "notifications":{
                        "type":"object", "additionalProperties":false,
                        "properties":{
                            "enabled":{"type":"boolean"},
                            "system":{"type":"boolean"},
                            "position":{"type":"string","enum":["top-left","top","top-right","right","bottom-right","bottom","bottom-left","left"]}
                        }
                    },
                    "mail":{
                        "type":"object", "additionalProperties":false,
                        "properties":{
                            "remote_images":{"type":"boolean"},
                            "prefetch":{
                                "type":"object", "additionalProperties":false,
                                "properties":{
                                    "enabled":{"type":"boolean"},
                                    "count":{"type":"integer","minimum":0,"maximum":100}
                                }
                            }
                        }
                    }
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

fn to_json<T: serde::Serialize>(value: T) -> Result<Value, Failure> {
    serde_json::to_value(value)
        .map_err(|_| Failure::from(ServiceError::conflict("Cannot serialize mail response")))
}

impl MailMcp {
    async fn execute(
        &self,
        principal: Principal,
        name: &str,
        input: Input,
    ) -> Result<Value, Failure> {
        let account = input.account;
        let id = input.id.unwrap_or_default();
        match name {
            "messages.flags" | "messages.move" | "messages.archive" | "messages.delete" => {
                self.service
                    .organize_messages(
                        &principal,
                        &account,
                        name.trim_start_matches("messages."),
                        OrganizeInput {
                            ids: input.ids.unwrap_or_default(),
                            seen: input.seen,
                            flagged: input.flagged,
                            folder: input.folder,
                        },
                    )
                    .await?;
                Ok(json!({ "ok": true }))
            }
            "account.read" => to_json(self.service.account_connected(&principal, &account).await?),
            "folders.list" => to_json(self.service.folders(&principal, &account).await?),
            "messages.list" | "messages.refresh" => to_json(
                self.service
                    .messages(
                        &principal,
                        &account,
                        MessageQuery {
                            folder: input.folder.unwrap_or_else(|| "inbox".into()),
                            q: input.q,
                            offset: input.offset,
                        },
                    )
                    .await?,
            ),
            "messages.cached" => to_json(
                self.service
                    .cached_messages(
                        &principal,
                        &account,
                        MessageQuery {
                            folder: input.folder.unwrap_or_else(|| "inbox".into()),
                            q: input.q,
                            offset: input.offset,
                        },
                    )
                    .await?,
            ),
            "messages.read" => to_json(self.service.message(&principal, &account, &id).await?),
            "drafts.list" => to_json(self.service.drafts(&principal, &account).await?),
            "drafts.read" => to_json(self.service.draft(&principal, &account, &id).await?),
            "drafts.write" => {
                let draft = input
                    .draft
                    .ok_or_else(|| Failure::from(ServiceError::conflict("Draft required")))?;
                to_json(self.service.save_draft(&principal, &account, draft).await?)
            }
            "labels.list" => to_json(self.service.labels(&principal, &account).await?),
            "messages.label" => {
                let label = input
                    .label
                    .ok_or_else(|| Failure::from(ServiceError::conflict("Label required")))?;
                to_json(
                    self.service
                        .update_label(&principal, &account, label)
                        .await?,
                )
            }
            "messages.send" => {
                let draft = input
                    .draft
                    .ok_or_else(|| Failure::from(ServiceError::conflict("Draft required")))?;
                to_json(self.service.send(&principal, &account, draft).await?)
            }
            "app.navigate" => {
                let target = input
                    .target
                    .filter(|t| NAVIGATION_TARGETS.contains(&t.as_str()))
                    .ok_or_else(|| {
                        Failure::from(ServiceError::conflict("Unknown navigation target"))
                    })?;
                to_json(self.service.navigate(&principal, &account, &target)?)
            }
            "settings.read" | "settings.update" => {
                let service = self.service.clone();
                let settings = input.settings;
                let update = name == "settings.update";
                tokio::task::spawn_blocking(move || match settings {
                    Some(settings) if update => {
                        service.update_scoped_settings(&principal, &account, settings)
                    }
                    None if !update => service.read_scoped_settings(&principal, &account),
                    _ => Err(ServiceError::conflict("Invalid settings tool input")),
                })
                .await
                .map_err(|_| Failure::forbidden())?
                .map_err(Failure::from)
            }
            "send.status" => to_json(self.service.send_request_status(
                &principal,
                &id,
                Some(&account),
            )?),
            _ => Err(Failure::from(ServiceError::conflict("Unknown mail tool"))),
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
                .ok_or_else(Failure::forbidden)?;
            let permit = self.service.try_acquire_execution()?;
            let input: Input =
                serde_json::from_value(Value::Object(request.arguments.unwrap_or_default()))
                    .map_err(|_| {
                        Failure::from(ServiceError::conflict("Invalid mail tool input"))
                    })?;
            if input.account.is_empty()
                || input.account.len() > 128
                || input.id.as_ref().is_some_and(|id| id.len() > 256)
            {
                return Err(Failure::from(ServiceError::conflict(
                    "Invalid mail tool input",
                )));
            }
            if matches!(
                request.name.as_ref(),
                "messages.read" | "drafts.read" | "send.status"
            ) && input.id.as_ref().is_none_or(String::is_empty)
            {
                return Err(Failure::from(ServiceError::conflict(
                    "Message or request ID required",
                )));
            }
            let mcp = self.clone();
            // Keep execution bounded even when the client disconnects during provider work.
            tokio::spawn(async move {
                let _permit = permit;
                mcp.execute(principal, &request.name, input).await
            })
            .await
            .map_err(|_| Failure::from(ServiceError::conflict("Mail service unavailable")))?
        }
        .await;
        Ok(match result {
            Ok(value) => CallToolResult::structured(value),
            Err(error) => {
                let value = json!({
                    "category": error.category,
                    "error": error.message,
                    "status": error.status.as_u16()
                });
                CallToolResult::structured_error(value)
            }
        }
        .into())
    }
}

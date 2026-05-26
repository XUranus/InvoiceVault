use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tracing::warn;

use crate::agent::{
    self, AgentArtifact, AgentAttachment, AgentMessageRow, AgentResponse, AgentSession, AgentTask,
    ConfirmRequest,
};
use crate::app_core::AppState;
use crate::llm::{self, LlmProviderConfig};

#[derive(Debug, Clone, Serialize)]
pub struct AgentStreamPayload {
    stream_id: String,
    session_id: i64,
    #[serde(flatten)]
    event: agent::AgentStreamEvent,
}

fn make_agent_stream_sink(
    app: AppHandle,
    stream_id: String,
    session_id: i64,
) -> agent::AgentStreamSink {
    Arc::new(move |event| {
        let payload = AgentStreamPayload {
            stream_id: stream_id.clone(),
            session_id,
            event,
        };
        if let Err(e) = app.emit("agent://stream", payload) {
            warn!("Failed to emit agent stream event: {e}");
        }
    })
}

#[tauri::command]
pub fn create_agent_session(state: State<'_, AppState>) -> Result<AgentSession, String> {
    state.create_agent_session().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_agent_sessions(state: State<'_, AppState>) -> Result<Vec<AgentSession>, String> {
    state.list_agent_sessions().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_agent_session(
    state: State<'_, AppState>,
    session_id: i64,
) -> Result<Vec<AgentMessageRow>, String> {
    state
        .get_agent_session(session_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn delete_agent_session(state: State<'_, AppState>, session_id: i64) -> Result<(), String> {
    state
        .delete_agent_session(session_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn update_agent_session_title(
    state: State<'_, AppState>,
    session_id: i64,
    title: String,
) -> Result<AgentSession, String> {
    state
        .update_agent_session_title(session_id, &title)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn send_agent_message(
    state: State<'_, AppState>,
    session_id: i64,
    content: String,
    attachment_ids: Option<Vec<i64>>,
    config: LlmProviderConfig,
) -> Result<AgentResponse, String> {
    state
        .send_agent_message(
            session_id,
            &content,
            attachment_ids.unwrap_or_default(),
            &config,
        )
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn send_agent_message_stream(
    app: AppHandle,
    state: State<'_, AppState>,
    stream_id: String,
    session_id: i64,
    content: String,
    attachment_ids: Option<Vec<i64>>,
    config: LlmProviderConfig,
) -> Result<AgentResponse, String> {
    let sink = make_agent_stream_sink(app, stream_id, session_id);
    sink(agent::AgentStreamEvent::Started);
    let result = state
        .send_agent_message_stream(
            session_id,
            &content,
            attachment_ids.unwrap_or_default(),
            &config,
            Arc::clone(&sink),
        )
        .await;
    match result {
        Ok(response) => {
            sink(agent::AgentStreamEvent::Finished);
            Ok(response)
        }
        Err(err) => {
            let message = err.to_string();
            sink(agent::AgentStreamEvent::Error {
                message: message.clone(),
            });
            Err(message)
        }
    }
}

#[tauri::command]
pub fn attach_agent_file(
    state: State<'_, AppState>,
    session_id: i64,
    path: String,
) -> Result<AgentAttachment, String> {
    state
        .attach_agent_file(session_id, &path)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_agent_attachments(
    state: State<'_, AppState>,
    session_id: i64,
) -> Result<Vec<AgentAttachment>, String> {
    state
        .list_agent_attachments(session_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn remove_agent_attachment(
    state: State<'_, AppState>,
    attachment_id: i64,
) -> Result<(), String> {
    state
        .remove_agent_attachment(attachment_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_agent_tasks(state: State<'_, AppState>, session_id: i64) -> Result<Vec<AgentTask>, String> {
    state
        .list_agent_tasks(session_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_agent_artifacts(
    state: State<'_, AppState>,
    session_id: i64,
) -> Result<Vec<AgentArtifact>, String> {
    state
        .list_agent_artifacts(session_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn open_agent_artifact_file(
    state: State<'_, AppState>,
    session_id: i64,
    artifact_id: i64,
) -> Result<(), String> {
    state
        .open_agent_artifact_file(session_id, artifact_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn open_agent_artifact_folder(
    state: State<'_, AppState>,
    session_id: i64,
    artifact_id: i64,
) -> Result<(), String> {
    state
        .open_agent_artifact_folder(session_id, artifact_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn delete_agent_artifact(
    state: State<'_, AppState>,
    session_id: i64,
    artifact_id: i64,
) -> Result<(), String> {
    state
        .delete_agent_artifact(session_id, artifact_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn confirm_agent_action(
    state: State<'_, AppState>,
    request: ConfirmRequest,
    config: LlmProviderConfig,
) -> Result<AgentResponse, String> {
    state
        .confirm_agent_action(request, &config)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn confirm_agent_action_stream(
    app: AppHandle,
    state: State<'_, AppState>,
    stream_id: String,
    request: ConfirmRequest,
    config: LlmProviderConfig,
) -> Result<AgentResponse, String> {
    let session_id = request.session_id;
    let sink = make_agent_stream_sink(app, stream_id, session_id);
    sink(agent::AgentStreamEvent::Started);
    let result = state
        .confirm_agent_action_stream(request, &config, Arc::clone(&sink))
        .await;
    match result {
        Ok(response) => {
            sink(agent::AgentStreamEvent::Finished);
            Ok(response)
        }
        Err(err) => {
            let message = err.to_string();
            sink(agent::AgentStreamEvent::Error {
                message: message.clone(),
            });
            Err(message)
        }
    }
}

#[tauri::command]
pub async fn generate_session_title(
    state: State<'_, AppState>,
    session_id: i64,
    config: LlmProviderConfig,
) -> Result<String, String> {
    let first_msg = {
        let db = state.db().lock().unwrap_or_else(|e| e.into_inner());
        agent::get_first_user_message(&db, session_id)
    };
    let first_msg = match first_msg {
        Some(msg) => msg,
        None => return Ok("新对话".to_owned()),
    };
    let title = llm::generate_title(&config, &first_msg)
        .await
        .map_err(|e| e.to_string())?;
    {
        let db = state.db().lock().unwrap_or_else(|e| e.into_inner());
        agent::set_session_title(&db, session_id, &title).map_err(|e| e.to_string())?;
    }
    Ok(title)
}

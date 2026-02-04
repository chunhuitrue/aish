use std::sync::Arc;

use rmcp::ClientHandler;
use rmcp::RoleClient;
use rmcp::model::CancelledNotificationParam;
use rmcp::model::ClientInfo;
use rmcp::model::CreateElicitationRequestParams;
use rmcp::model::CreateElicitationResult;
use rmcp::model::LoggingLevel;
use rmcp::model::LoggingMessageNotificationParam;
use rmcp::model::ProgressNotificationParam;
use rmcp::model::RequestId;
use rmcp::model::ResourceUpdatedNotificationParam;
use rmcp::service::NotificationContext;
use rmcp::service::RequestContext;
use tracing::info;

use crate::rmcp_client::SendElicitation;

#[derive(Clone)]
pub(crate) struct LoggingClientHandler {
    client_info: ClientInfo,
    send_elicitation: Arc<SendElicitation>,
}

impl LoggingClientHandler {
    pub(crate) fn new(client_info: ClientInfo, send_elicitation: SendElicitation) -> Self {
        Self {
            client_info,
            send_elicitation: Arc::new(send_elicitation),
        }
    }
}

impl ClientHandler for LoggingClientHandler {
    async fn create_elicitation(
        &self,
        request: CreateElicitationRequestParams,
        context: RequestContext<RoleClient>,
    ) -> Result<CreateElicitationResult, rmcp::ErrorData> {
        let id = match context.id {
            RequestId::String(id) => mcp_types::RequestId::String(id.to_string()),
            RequestId::Number(id) => mcp_types::RequestId::Integer(id),
        };
        (self.send_elicitation)(id, request)
            .await
            .map_err(|err| rmcp::ErrorData::internal_error(err.to_string(), None))
    }

    async fn on_logging_message(
        &self,
        params: LoggingMessageNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        match params.level {
            LoggingLevel::Debug => tracing::debug!("{}", params.data),
            LoggingLevel::Info => tracing::info!("{}", params.data),
            LoggingLevel::Notice | LoggingLevel::Warning => tracing::warn!("{}", params.data),
            LoggingLevel::Error
            | LoggingLevel::Critical
            | LoggingLevel::Alert
            | LoggingLevel::Emergency => tracing::error!("{}", params.data),
        }
    }

    async fn on_progress(
        &self,
        _params: ProgressNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
    }

    async fn on_resource_updated(
        &self,
        _params: ResourceUpdatedNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
    }

    async fn on_cancelled(
        &self,
        _params: CancelledNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
    }

    async fn on_resource_list_changed(&self, _context: NotificationContext<RoleClient>) {
        info!("MCP server resource list changed");
    }

    async fn on_tool_list_changed(&self, _context: NotificationContext<RoleClient>) {
        info!("MCP server tool list changed");
    }

    async fn on_prompt_list_changed(&self, _context: NotificationContext<RoleClient>) {
        info!("MCP server prompt list changed");
    }

    fn get_info(&self) -> ClientInfo {
        self.client_info.clone()
    }
}

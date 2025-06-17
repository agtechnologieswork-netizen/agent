"""
Implementation of a minimal dummy template agent for testing purposes.

This implementation provides a simple agent that returns basic diff output
without complex generation logic, useful for testing the template system.
"""
import dagger
from typing import Dict, Any, Optional
from anyio.streams.memory import MemoryObjectSendStream

from api.agent_server.interface import AgentInterface
from api.agent_server.models import AgentSseEvent, AgentMessage, AgentStatus, MessageKind, ExternalContentBlock

from log import get_logger

logger = get_logger(__name__)


class DummyTemplateAgentImplementation(AgentInterface):
    """
    Minimal dummy template agent implementation for testing purposes.
    
    This agent generates a simple "Hello World" application with minimal
    server and client files to demonstrate the template system functionality.
    """

    def __init__(self, client: dagger.Client, application_id: str, trace_id: str, settings: Optional[Dict[str, Any]] = None):
        """
        Initialize the DummyTemplateAgentImplementation.

        Args:
            client: Dagger client for containerized operations
            application_id: Unique identifier for the application
            trace_id: Trace ID for tracking the request
            settings: Optional settings for the agent
        """
        self.client = client
        self.application_id = application_id
        self.trace_id = trace_id
        self.settings = settings or {}
        self.state = {}
        logger.info(f"Initialized dummy template agent for {application_id}:{trace_id}")

    async def process(self, request, event_tx: MemoryObjectSendStream[AgentSseEvent]) -> None:
        """
        Process the agent request and generate a simple dummy application.

        Args:
            request: The agent request containing messages and context
            event_tx: Channel to send events back to the client
        """
        try:
            logger.info(f"Processing dummy template request for {self.application_id}:{self.trace_id}")

            user_message = request.all_messages[-1].content if request.all_messages else "Create a dummy app"

            await event_tx.send(AgentSseEvent(
                status=AgentStatus.RUNNING,
                traceId=self.trace_id,
                message=AgentMessage(
                    role="assistant",
                    kind=MessageKind.STAGE_RESULT,
                    messages=[ExternalContentBlock(content="Starting dummy app generation...")],
                    agentState=self.state,
                    unifiedDiff=None
                )
            ))

            unified_diff = self._generate_dummy_diff(user_message)

            self.state = {
                "template_type": "dummy",
                "user_request": user_message,
                "files_generated": 2,
                "status": "completed"
            }

            await event_tx.send(AgentSseEvent(
                status=AgentStatus.IDLE,
                traceId=self.trace_id,
                message=AgentMessage(
                    role="assistant",
                    kind=MessageKind.REVIEW_RESULT,
                    messages=[ExternalContentBlock(content="Dummy application generated successfully! This is a minimal template for testing purposes.")],
                    agentState=self.state,
                    unifiedDiff=unified_diff,
                    app_name="dummy-app",
                    commit_message="Initial dummy template commit"
                )
            ))

        except Exception as e:
            logger.exception(f"Error processing dummy template request: {str(e)}")

            await event_tx.send(AgentSseEvent(
                status=AgentStatus.IDLE,
                traceId=self.trace_id,
                message=AgentMessage(
                    role="assistant",
                    kind=MessageKind.RUNTIME_ERROR,
                    messages=[ExternalContentBlock(content=f"Error generating dummy application: {str(e)}")],
                    agentState=self.state,
                    unifiedDiff=None
                )
            ))

        finally:
            await event_tx.aclose()

    def _generate_dummy_diff(self, user_message: str) -> str:
        """
        Generate a simple unified diff for the dummy template.

        Args:
            user_message: The user's request message

        Returns:
            A unified diff string showing the dummy application files
        """
        logger.info(f"Generating dummy diff based on: {user_message}")

        unified_diff = """diff --git a/server/index.js b/server/index.js
new file mode 100644
index 0000000..1234567
--- /dev/null
+++ b/server/index.js
@@ -0,0 +1,10 @@
+const express = require('express');
+const app = express();
+const port = 3000;
+
+app.get('/', (req, res) => {
+  res.json({ message: 'Hello from dummy template!', request: '""" + user_message.replace("'", "\\'") + """' });
+});
+
+app.listen(port, () => {
+  console.log(`Dummy server running at http://localhost:${port}`);
+});
diff --git a/client/index.html b/client/index.html
new file mode 100644
index 0000000..2345678
--- /dev/null
+++ b/client/index.html
@@ -0,0 +1,15 @@
+<!DOCTYPE html>
+<html lang="en">
+<head>
+    <meta charset="UTF-8">
+    <meta name="viewport" content="width=device-width, initial-scale=1.0">
+    <title>Dummy Template App</title>
+</head>
+<body>
+    <h1>Dummy Template Application</h1>
+    <p>This is a minimal dummy template for testing purposes.</p>
+    <p>User request: """ + user_message + """</p>
+    <button onclick="alert('Dummy template works!')">Test Button</button>
+</body>
+</html>
"""

        return unified_diff

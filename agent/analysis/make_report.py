"""
Error Analysis Report Generator

This script extracts error information from production logs and generates
comprehensive reports with trace data from S3.

Usage:
    uv run analysis/make_report.py partial_trace_id
    uv run analysis/make_report.py  # Uses latest error from prod logs
"""

import re
import json
import boto3
import time
import subprocess
import logging
from datetime import datetime, timedelta
from pathlib import Path
from typing import Dict, List, Any, Optional, Tuple
from collections import defaultdict
from dataclasses import dataclass
from joblib import Memory
import fire
import shutil

from analysis.trace_loader import TraceLoader
from analysis.utils import extract_trajectories_from_dump

logger = logging.getLogger(__name__)


@dataclass
class Config:
    """Configuration for the error report generator."""
    s3_bucket: str = "prod-agent-service-snapshots"
    log_group: str = "/ecs/prod-agent-service/application"
    cache_dir: Path = Path("/tmp/error_report_cache")
    query_timeout_seconds: int = 30
    max_log_hours_back: int = 24
    reports_dir: Path = Path("/tmp")


class CloudWatchLogFetcher:
    """Handles all CloudWatch log operations."""

    def __init__(self, log_group: str):
        self.log_group = log_group
        self.logs_client = boto3.client('logs')

    def extract_trace_id_from_log(self, log_message: str) -> Optional[str]:
        """Extract trace_id from error log message."""
        # pattern for trace_id in the format: app-{uuid}.req-{uuid}
        patterns = [
            r'"trace_id":"(app-[a-f0-9-]+\.req-[a-f0-9-]+)"',
            r'trace_id[=:]\s*(app-[a-f0-9-]+\.req-[a-f0-9-]+)'
        ]

        for pattern in patterns:
            match = re.search(pattern, log_message)
            if match:
                return match.group(1)
        return None

    def _execute_query(self, query: str, start_time_ms: int, end_time_ms: int, timeout: int) -> Optional[List[Dict]]:
        """Execute a CloudWatch Insights query and wait for results."""
        response = self.logs_client.start_query(
            logGroupName=self.log_group,
            startTime=start_time_ms,
            endTime=end_time_ms,
            queryString=query
        )

        query_id = response['queryId']
        waited = 0

        while waited < timeout:
            result = self.logs_client.get_query_results(queryId=query_id)
            status = result['status']

            if status == 'Complete':
                return result.get('results', [])
            elif status == 'Failed':
                logger.error("CloudWatch query failed")
                return None

            time.sleep(1)
            waited += 1

        logger.error("CloudWatch query timed out")
        return None

    def get_latest_error_trace_id(self, hours_back: int = 24, timeout: int = 30) -> Tuple[Optional[str], Optional[str]]:
        """Get the latest error trace_id and error message from CloudWatch logs."""
        try:
            end_time = datetime.utcnow()
            start_time = end_time - timedelta(hours=hours_back)

            start_time_ms = int(start_time.timestamp() * 1000)
            end_time_ms = int(end_time.timestamp() * 1000)

            logger.info(f"Searching for errors in {self.log_group} from {start_time} to {end_time}")

            query = '''
            fields @timestamp, @message
            | filter @message like /ERROR/
            | filter @message like /trace_id/
            | sort @timestamp desc
            | limit 1
            '''

            results = self._execute_query(query, start_time_ms, end_time_ms, timeout)
            if not results:
                logger.warning(f"No error logs found in the last {hours_back} hours")
                return None, None

            # extract trace_id from the latest error
            latest_error = results[0]
            message_field = next((field for field in latest_error if field['field'] == '@message'), None)

            if message_field:
                error_message = message_field['value']
                trace_id = self.extract_trace_id_from_log(error_message)
                if trace_id:
                    logger.info(f"Found latest error trace_id: {trace_id}")
                    return trace_id, error_message
                else:
                    logger.warning("Could not extract trace_id from latest error log")
                    return None, error_message

        except Exception:
            logger.exception("Error querying CloudWatch logs")

        return None, None

    def get_error_logs_for_trace(self, trace_id: str, hours_back: int = 24, timeout: int = 30) -> List[Dict[str, Any]]:
        """Get all error logs for a specific trace_id from CloudWatch."""
        try:
            end_time = datetime.utcnow()
            start_time = end_time - timedelta(hours=hours_back)

            start_time_ms = int(start_time.timestamp() * 1000)
            end_time_ms = int(end_time.timestamp() * 1000)

            logger.info(f"Searching for all error logs with trace_id: {trace_id}")

            query = f'''
            fields @timestamp, @message
            | filter @message like /{trace_id}/
            | filter @message like /ERROR/ or @message like /WARN/
            | sort @timestamp asc
            '''

            results = self._execute_query(query, start_time_ms, end_time_ms, timeout)
            if not results:
                return []

            error_logs = []
            for log_entry in results:
                timestamp_field = next((field for field in log_entry if field['field'] == '@timestamp'), None)
                message_field = next((field for field in log_entry if field['field'] == '@message'), None)

                if message_field and timestamp_field:
                    error_logs.append({
                        'timestamp': timestamp_field['value'],
                        'message': message_field['value']
                    })

            logger.info(f"Found {len(error_logs)} error/warning logs for trace_id: {trace_id}")
            return error_logs

        except Exception:
            logger.exception("Error querying CloudWatch logs")
            return []


class S3TraceFetcher:
    """Handles S3 trace file operations."""

    def __init__(self, s3_bucket: str, cache_wrapper: Optional['CachedWrapper'] = None):
        self.s3_bucket = s3_bucket
        self.trace_loader = TraceLoader(s3_bucket)
        self.cache_wrapper = cache_wrapper

    def find_trace_files(self, partial_trace_id: str) -> Dict[str, List[Dict[str, Any]]]:
        """Find all trace files related to a partial trace_id."""
        if self.cache_wrapper:
            return self.cache_wrapper.cache(self._find_trace_files_impl)(partial_trace_id)
        return self._find_trace_files_impl(partial_trace_id)

    def _find_trace_files_impl(self, partial_trace_id: str) -> Dict[str, List[Dict[str, Any]]]:
        """Implementation of find_trace_files."""
        trace_files = {
            'fsm_exit': [],
            'fsm_enter': [],
            'sse_events': [],
            'fsmtools_messages': []
        }

        patterns = {
            'fsm_exit': ['*fsm_exit.json'],
            'fsm_enter': ['*fsm_enter.json'],
            'sse_events': ['*sse_events*'],
            'fsmtools_messages': ['*fsmtools_messages.json']
        }

        for trace_type, file_patterns in patterns.items():
            logger.info(f"Searching for {trace_type} files matching {partial_trace_id}")

            files = self.trace_loader.list_trace_files(file_patterns)

            # filter files that match the partial trace_id
            matching_files = [
                file_info for file_info in files
                if partial_trace_id in file_info.get('path', '') or
                   partial_trace_id in file_info.get('name', '')
            ]

            trace_files[trace_type] = matching_files
            logger.info(f"Found {len(matching_files)} {trace_type} files")

        return trace_files

    def load_file(self, file_info: Dict[str, Any]) -> Any:
        """Load a file with caching support."""
        if self.cache_wrapper:
            # Pass all relevant file_info fields for caching
            return self.cache_wrapper.cache(self._load_file_impl)(
                file_info['path'],
                file_info['name'],
                file_info.get('is_local', False)
            )
        return self.trace_loader.load_file(file_info)

    def _load_file_impl(self, file_path: str, file_name: str, is_local: bool) -> Any:
        """Implementation of load_file for caching."""
        file_info = {'path': file_path, 'name': file_name, 'is_local': is_local}
        return self.trace_loader.load_file(file_info)

    def group_sse_events(self, sse_files: List[Dict[str, Any]]) -> Dict[str, List[Dict[str, Any]]]:
        """Group SSE event files by trace ID and sort by sequence number."""
        trace_groups = defaultdict(list)

        for file_info in sse_files:
            # s3 pattern: app-{app_id}.req-{req_id}_{timestamp}/sse_events/{sequence}.json
            path = file_info["path"]
            match = re.match(r"(app-[a-f0-9-]+\.req-[a-f0-9-]+)_\d+/sse_events/(\d+)\.json", path)
            if match:
                trace_id, sequence = match.groups()
                file_info["trace_id"] = trace_id
                file_info["sequence"] = int(sequence)
                trace_groups[trace_id].append(file_info)

        # sort each group by sequence number
        for trace_id in trace_groups:
            trace_groups[trace_id].sort(key=lambda x: x["sequence"])

        return dict(trace_groups)


class MessageFormatter:
    """Formats different types of messages for markdown output."""

    @staticmethod
    def format_tool_use(name: str, input_data: Dict) -> List[str]:
        """Format tool use message."""
        return [
            f"**Tool Use:** {name}",
            "```json",
            json.dumps(input_data, indent=2),
            "```"
        ]

    @staticmethod
    def format_text_content(text: str) -> List[str]:
        """Format text content with proper escaping."""
        return [
            "**Text:**",
            "```markdown",
            text,
            "```"
        ]

    @staticmethod
    def format_tool_result(result: Dict) -> List[str]:
        """Format tool result."""
        return [
            "**Tool Result:**",
            "```json",
            json.dumps(result, indent=2),
            "```"
        ]

    @staticmethod
    def format_json_content(content: Any) -> List[str]:
        """Format generic JSON content."""
        return [
            "```json",
            json.dumps(content, indent=2) if not isinstance(content, str) else content,
            "```"
        ]


class MarkdownReportBuilder:
    """Builds markdown reports from data."""

    def __init__(self):
        self.sections = []
        self.formatter = MessageFormatter()

    def add_header(self, trace_id: str) -> 'MarkdownReportBuilder':
        """Add report header."""
        self.sections.extend([
            "# Error Analysis Report",
            f"**Trace ID:** `{trace_id}`",
            f"**Generated:** {datetime.now().strftime('%Y-%m-%d %H:%M:%S UTC')}",
            ""
        ])
        return self

    def add_cloudwatch_logs(self, logs: List[Dict[str, Any]]) -> 'MarkdownReportBuilder':
        """Add CloudWatch logs section."""
        if not logs:
            return self

        self.sections.extend([
            "## CloudWatch Logs (Errors & Warnings)",
            f"*Found {len(logs)} error/warning log(s) from CloudWatch for this trace.*",
            ""
        ])

        for i, error_log in enumerate(logs):
            if i > 0:
                self.sections.append("---")

            self._add_single_cloudwatch_log(error_log, i + 1)

        self.sections.extend(["---", ""])
        return self

    def _add_single_cloudwatch_log(self, error_log: Dict[str, Any], index: int) -> None:
        """Add a single CloudWatch log entry."""
        timestamp = error_log.get('timestamp', 'Unknown')
        error_message = error_log.get('message', '')

        self.sections.extend([
            f"### Log Entry #{index}",
            f"**CloudWatch Timestamp:** `{timestamp}`",
            ""
        ])

        try:
            # try to parse the error message as JSON for better formatting
            error_data = json.loads(error_message)
            self._add_parsed_error_data(error_data)
        except (json.JSONDecodeError, TypeError):
            # fallback to raw message
            self.sections.extend([
                "#### Raw CloudWatch Log Entry",
                "```json",
                error_message,
                "```"
            ])

        self.sections.append("")

    def _add_parsed_error_data(self, error_data: Dict[str, Any]) -> None:
        """Add parsed error data."""
        # display key error information
        self.sections.extend([
            f"**Application Timestamp:** `{error_data.get('timestamp', 'Unknown')}`",
            f"**Logger:** `{error_data.get('logger', 'Unknown')}`"
        ])

        # determine log level
        level = error_data.get('level', 'ERROR')
        self.sections.append(f"**Level:** `{level}`")

        # error message
        if 'message' in error_data:
            self.sections.extend([
                "",
                "#### Error Message",
                "```",
                error_data.get('message', 'Unknown'),
                "```"
            ])

        # exception details
        if 'exception' in error_data:
            self.sections.extend([
                "",
                "#### Exception Stack Trace",
                "```python",
                error_data['exception'],
                "```"
            ])

        # additional context fields
        context_fields = {
            k: v for k, v in error_data.items()
            if k not in ['timestamp', 'logger', 'message', 'exception', 'trace_id', 'level']
        }
        if context_fields:
            self.sections.extend([
                "",
                "#### Additional Context",
                "```json",
                json.dumps(context_fields, indent=2),
                "```"
            ])

    def add_summary(self, trace_files: Dict[str, List[Dict[str, Any]]]) -> 'MarkdownReportBuilder':
        """Add summary section."""
        total_files = sum(len(files) for files in trace_files.values())
        self.sections.extend([
            "## Summary",
            f"- **Total trace files found:** {total_files}"
        ])

        for trace_type, files in trace_files.items():
            if files:
                self.sections.append(
                    f"- **{trace_type.replace('_', ' ').title()}:** {len(files)} files"
                )

        self.sections.append("")
        return self

    def add_fsm_exit_states(self, files: List[Dict[str, Any]], loader: S3TraceFetcher) -> 'MarkdownReportBuilder':
        """Add FSM exit states section."""
        if not files:
            return self

        self.sections.append("## FSM Exit States")

        for file_info in files:
            try:
                content = loader.load_file(file_info)
                self._add_fsm_exit_file(file_info, content)
            except Exception:
                logger.exception(f"Error processing FSM exit file {file_info['name']}")
                self.sections.append(f"Error loading file: {file_info['name']}")

        self.sections.append("")
        return self

    def _add_fsm_exit_file(self, file_info: Dict[str, Any], content: Any) -> None:
        """Add a single FSM exit file."""
        self.sections.extend([
            f"### {file_info['name']}",
            f"**Modified:** {file_info['modified']}",
            ""
        ])

        # extract trajectories
        trajectories = extract_trajectories_from_dump(content)
        self.sections.append(f"**Trajectories found:** {len(trajectories)}")

        for traj_name, messages in trajectories.items():
            self._add_trajectory(traj_name, messages)

    def _add_trajectory(self, traj_name: str, messages: List[Dict[str, Any]]) -> None:
        """Add a single trajectory."""
        self.sections.extend([
            f"#### Trajectory: `{traj_name}`",
            f"Messages: {len(messages)}",
            "",
            "<details>",
            f"<summary>Click to expand trajectory details</summary>",
            ""
        ])

        for i, msg in enumerate(messages):
            self._add_trajectory_message(msg, i + 1)

        self.sections.extend([
            "</details>",
            "",
            "---"
        ])

    def _add_trajectory_message(self, msg: Dict[str, Any], index: int) -> None:
        """Add a single trajectory message."""
        role = msg.get('role', 'unknown')
        content_list = msg.get('content', [])
        self.sections.append(f"##### Message {index}: {role}")

        if content_list and isinstance(content_list, list):
            for item in content_list:
                if isinstance(item, dict):
                    self._add_message_content_item(item)
        else:
            self.sections.extend([
                "**Content:**",
                *self.formatter.format_json_content(content_list)
            ])

        self.sections.append("")

    def _add_message_content_item(self, item: Dict[str, Any]) -> None:
        """Add a single message content item."""
        item_type = item.get('type')

        if item_type == 'text':
            self.sections.extend(
                self.formatter.format_text_content(item.get('text', ''))
            )
        elif item_type == 'tool_use':
            self.sections.extend(
                self.formatter.format_tool_use(
                    item.get('name', 'Unknown'),
                    item.get('input', {})
                )
            )
        elif item_type == 'tool_use_result':
            self.sections.extend(
                self.formatter.format_tool_result(item.get('tool_result', {}))
            )
        else:
            self.sections.extend([
                "**Other Content:**",
                *self.formatter.format_json_content(item)
            ])

    def add_sse_events(self, files: List[Dict[str, Any]], loader: S3TraceFetcher) -> 'MarkdownReportBuilder':
        """Add SSE events section."""
        if not files:
            return self

        self.sections.append("## SSE Events")

        # group SSE events by trace ID
        sse_groups = loader.group_sse_events(files)

        for group_trace_id, events in sse_groups.items():
            self._add_sse_group(group_trace_id, events, loader)

        return self

    def _add_sse_group(self, trace_id: str, events: List[Dict[str, Any]], loader: S3TraceFetcher) -> None:
        """Add a group of SSE events."""
        self.sections.extend([
            f"### Trace: {trace_id}",
            f"**Total events:** {len(events)}",
            ""
        ])

        # load and display events
        loaded_events = []
        for file_info in events:
            try:
                event_content = loader.load_file(file_info)
                event_content["sequence"] = file_info["sequence"]
                loaded_events.append(event_content)
            except Exception:
                logger.exception(f"Error loading SSE event {file_info['name']}")

        # sort by sequence
        loaded_events.sort(key=lambda x: x.get("sequence", 0))

        for event in loaded_events:
            self._add_sse_event(event)

    def _add_sse_event(self, event: Dict[str, Any]) -> None:
        """Add a single SSE event."""
        seq = event.get("sequence", 0)
        status = event.get("status", "unknown")
        message = event.get("message", {})
        kind = message.get("kind", "Unknown")

        self.sections.extend([
            f"#### Event {seq}: {status.upper()}",
            f"**Kind:** {kind}"
        ])

        # show content
        content = message.get("content", "")
        if content:
            self.sections.extend([
                "**Content:**",
                *self.formatter.format_json_content(content)
            ])

        # show agent state if available
        agent_state = message.get("agent_state", {})
        if agent_state:
            state_summary = {
                "current_state": agent_state.get("current_state"),
                "error_count": len(agent_state.get("errors", [])),
                "message_count": len(agent_state.get("fsm_messages", [])),
            }
            self.sections.extend([
                "**Agent State:**",
                *self.formatter.format_json_content(state_summary)
            ])

        # show unified diff if available
        unified_diff = message.get("unified_diff")
        if unified_diff:
            self.sections.extend([
                "**Unified Diff:**",
                "```diff",
                unified_diff,
                "```"
            ])

        top_level_messages = message.get("messages")
        if top_level_messages:
            self.sections.extend(
                ["**Top level messages**",
                *self.formatter.format_json_content(top_level_messages)
                ]
            )

        self.sections.append("")


    def build(self) -> str:
        """Build the final markdown report."""
        return "\n".join(self.sections)


class HtmlConverter:
    """Converts markdown to HTML."""

    CUSTOM_CSS = """
<style>
body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    line-height: 1.6;
    color: #333;
    max-width: 1200px;
    margin: 0 auto;
    padding: 20px;
    background-color: #f8f9fa;
}
h1, h2, h3, h4, h5 {
    color: #2c3e50;
    margin-top: 1.5em;
}
h1 { border-bottom: 3px solid #3498db; padding-bottom: 0.3em; }
h2 { border-bottom: 2px solid #e0e0e0; padding-bottom: 0.2em; }
pre {
    background-color: #f4f4f4;
    border: 1px solid #ddd;
    border-radius: 4px;
    padding: 10px;
    overflow-x: auto;
    font-size: 0.9em;
    max-height: 600px;
    overflow-y: auto;
}
code {
    background-color: #f4f4f4;
    padding: 2px 4px;
    border-radius: 3px;
    font-family: 'Consolas', 'Monaco', monospace;
}
pre code {
    background-color: transparent;
    padding: 0;
}
strong { color: #2c3e50; }
ul, ol {
    padding-left: 30px;
    margin: 10px 0;
}
.highlight { background-color: #fff3cd; }
blockquote {
    border-left: 4px solid #3498db;
    padding-left: 15px;
    color: #666;
    margin: 10px 0;
}
details {
    background-color: #f9f9f9;
    border: 1px solid #e0e0e0;
    border-radius: 4px;
    padding: 10px;
    margin: 10px 0;
}
details summary {
    cursor: pointer;
    font-weight: bold;
    color: #3498db;
    padding: 5px;
}
details summary:hover {
    color: #2c3e50;
}
details[open] summary {
    margin-bottom: 10px;
    border-bottom: 1px solid #e0e0e0;
}
</style>
"""

    def convert(self, markdown_path: Path, trace_id: str) -> Optional[Path]:
        """Convert markdown file to HTML."""
        try:
            html_path = markdown_path.with_suffix('.html')

            result = subprocess.run(
                [
                    'pandoc', str(markdown_path), '-f', 'gfm', '-t', 'html', '-s',
                    '--highlight-style=pygments',
                    '--metadata', f'title=Error Analysis Report - {trace_id}',
                    '-H', '/dev/stdin', '-o', str(html_path)
                ],
                input=self.CUSTOM_CSS,
                capture_output=True,
                text=True
            )

            if result.returncode == 0:
                logger.info(f"HTML report saved to: {html_path}")
                return html_path
            else:
                logger.warning(f"Failed to generate HTML report: {result.stderr}")

        except FileNotFoundError:
            logger.info("Pandoc not found, skipping HTML generation")
        except Exception:
            logger.exception("Error generating HTML report")

        return None


class CachedWrapper:
    """Generic caching wrapper for any callable."""

    def __init__(self, cache_dir: Optional[Path] = None):
        if cache_dir:
            cache_dir.mkdir(exist_ok=True)
            self.memory = Memory(cache_dir, verbose=0)
        else:
            self.memory = Memory(None, verbose=0)

    def cache(self, func):
        """Decorator to cache function results."""
        return self.memory.cache(func)


class ErrorReportGenerator:
    """Orchestrates the error report generation process."""

    def __init__(
        self,
        config: Optional[Config] = None,
        use_cache: bool = True,
        cache_dir: Optional[Path] = None
    ):
        self.config = config or Config()
        if cache_dir:
            self.config.cache_dir = cache_dir

        # setup caching
        cache_wrapper = CachedWrapper(self.config.cache_dir) if use_cache else None

        # initialize components
        self.cloudwatch = CloudWatchLogFetcher(self.config.log_group)
        self.s3_fetcher = S3TraceFetcher(self.config.s3_bucket, cache_wrapper)
        self.html_converter = HtmlConverter()

    def create_report(self, partial_trace_id: Optional[str] = None) -> Path:
        """Create a comprehensive error report."""
        # get trace ID if not provided
        trace_id, initial_error = self._get_trace_id(partial_trace_id)

        # fetch all error logs
        error_logs = self._fetch_error_logs(trace_id, initial_error)

        # find trace files
        trace_files = self.s3_fetcher.find_trace_files(trace_id)

        # validate we found files
        total_files = sum(len(files) for files in trace_files.values())
        if total_files == 0:
            logger.warning(f"No trace files found for {trace_id}")
            return self._save_empty_report(trace_id)

        # generate markdown report
        markdown_content = self._build_markdown_report(trace_id, trace_files, error_logs)

        # save reports
        return self._save_reports(trace_id, markdown_content)

    def _get_trace_id(self, partial_trace_id: Optional[str]) -> Tuple[str, Optional[Dict[str, Any]]]:
        """Get trace ID and initial error if needed."""
        if partial_trace_id:
            return partial_trace_id, None

        logger.info("No trace_id provided, searching for latest error in CloudWatch logs...")
        trace_id, first_error_message = self.cloudwatch.get_latest_error_trace_id()

        if not trace_id:
            raise ValueError("No trace_id provided and could not find latest error from CloudWatch logs")

        initial_error = {
            'timestamp': 'Latest',
            'message': first_error_message
        } if first_error_message else None

        return trace_id, initial_error

    def _fetch_error_logs(self, trace_id: str, initial_error: Optional[Dict[str, Any]]) -> List[Dict[str, Any]]:
        """Fetch all error logs for the trace."""
        logger.info(f"Fetching all error logs for trace_id: {trace_id}")
        error_logs = self.cloudwatch.get_error_logs_for_trace(trace_id)

        # use initial error if no logs found
        if not error_logs and initial_error:
            error_logs = [initial_error]

        return error_logs

    def _build_markdown_report(
        self,
        trace_id: str,
        trace_files: Dict[str, List[Dict[str, Any]]],
        error_logs: List[Dict[str, Any]]
    ) -> str:
        """Build the markdown report."""
        builder = MarkdownReportBuilder()

        (builder
            .add_header(trace_id)
            .add_cloudwatch_logs(error_logs)
            .add_summary(trace_files)
            .add_fsm_exit_states(trace_files.get('fsm_exit', []), self.s3_fetcher)
            .add_sse_events(trace_files.get('sse_events', []), self.s3_fetcher)
        )

        return builder.build()

    def _save_empty_report(self, trace_id: str) -> Path:
        """Save an empty report when no files found."""
        content = f"No trace files found for trace_id: {trace_id}"
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        filename = f"error_report_{trace_id.replace('.', '_')}_{timestamp}.md"
        path = self.config.reports_dir / filename

        with open(path, 'w') as f:
            f.write(content)

        return path

    def _save_reports(self, trace_id: str, markdown_content: str) -> Path:
        """Save markdown and optionally HTML reports."""
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        filename = f"error_report_{trace_id.replace('.', '_')}_{timestamp}.md"
        markdown_path = self.config.reports_dir / filename

        # save markdown
        with open(markdown_path, 'w') as f:
            f.write(markdown_content)

        logger.info(f"Report saved to: {markdown_path}")

        # try to generate HTML
        self.html_converter.convert(markdown_path, trace_id)

        return markdown_path


def main(
    partial_trace_id: Optional[str] = None,
    s3_bucket: str = "prod-agent-service-snapshots",
    clear_cache: bool = False,
    use_cache: bool = True,
    cache_dir: Optional[str] = None
):
    """
    Generate error analysis report.

    Args:
        partial_trace_id: Partial trace ID to search for. If not provided, will use latest error from CloudWatch.
        s3_bucket: S3 bucket containing trace snapshots (default: prod-agent-service-snapshots)
        clear_cache: Clear the cache before running (default: False)
        use_cache: Whether to use caching for trace data (default: True)
        cache_dir: Custom cache directory (default: /tmp/error_report_cache)
    """
    cache_path = Path(cache_dir) if cache_dir else Config.cache_dir

    if clear_cache and cache_path.exists():
        logger.info(f"Clearing cache at {cache_path}...")
        shutil.rmtree(cache_path)

    config = Config(s3_bucket=s3_bucket, cache_dir=cache_path)
    generator = ErrorReportGenerator(config, use_cache=use_cache)
    report_path = generator.create_report(partial_trace_id)
    print(f"Report generated: {report_path}")


if __name__ == "__main__":
    fire.Fire(main)

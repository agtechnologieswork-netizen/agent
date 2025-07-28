from typing import List, Dict, Any, TypedDict, NotRequired, cast, Literal
import groq
from groq.types.chat import ChatCompletion
import json
from llm import common
from llm.telemetry import LLMTelemetry
from log import get_logger
import logging
from tenacity import retry, stop_after_attempt, wait_exponential_jitter, before_sleep_log, RetryCallState

logger = get_logger(__name__)


def is_rate_limit_error(retry_state: RetryCallState) -> bool:
    """check if the exception is a rate limit error or service unavailable."""
    if retry_state.outcome and retry_state.outcome.exception():
        exception = retry_state.outcome.exception()
        if hasattr(exception, 'status_code'):
            # Retry on both rate limit (429) and service unavailable (503) errors
            return exception.status_code in [429, 503]  # type: ignore
    return False


retry_rate_limits = retry(
    stop=stop_after_attempt(3),
    wait=wait_exponential_jitter(initial=1, max=30),
    retry=is_rate_limit_error,
    before_sleep=before_sleep_log(logger, logging.WARNING),
    retry_error_callback=lambda retry_state: logger.warning(f"Retry failed: {retry_state.outcome.exception() if retry_state.outcome else 'Unknown error'}")
)


class GroqParams(TypedDict):
    messages: List[Dict[str, Any]]
    model: str
    temperature: float
    max_tokens: int
    tools: NotRequired[List[Dict[str, Any]]]
    tool_choice: NotRequired[str]


class GroqLLM(common.AsyncLLM):
    def __init__(self, api_key: str | None = None, default_model: str = "moonshotai/kimi-k2-instruct"):
        self.client = groq.AsyncGroq(api_key=api_key)
        self.default_model = default_model

    def _messages_into(self, messages: List[common.Message]) -> List[Dict[str, Any]]:
        groq_messages = []
        for message in messages:
            content_parts = []
            tool_calls = []

            for block in message.content:
                match block:
                    case common.TextRaw(text) if text.strip():
                        content_parts.append(text.strip())
                    case common.ToolUse(name, input, id):
                        # groq API requires arguments as JSON string
                        args_str = json.dumps(input) if isinstance(input, dict) else str(input)
                        tool_calls.append({
                            "id": id or f"call_{name}",
                            "type": "function",
                            "function": {
                                "name": name,
                                "arguments": args_str
                            }
                        })
                    case common.ToolUseResult(tool_use, tool_result):
                        groq_messages.append({
                            "role": "tool",
                            "content": tool_result.content,
                            "tool_call_id": tool_use.id or f"call_{tool_use.name}"
                        })
                        continue

            if content_parts or tool_calls:
                groq_message: Dict[str, Any] = {
                    "role": message.role,
                    "content": " ".join(content_parts) if content_parts else ""
                }

                if tool_calls:
                    groq_message["tool_calls"] = tool_calls

                groq_messages.append(groq_message)

        return groq_messages

    def _tools_into(self, tools: List[common.Tool] | None) -> List[Dict[str, Any]] | None:
        if not tools:
            return None

        groq_tools = []
        for tool in tools:
            groq_tools.append({
                "type": "function",
                "function": {
                    "name": tool["name"],
                    "description": tool.get("description", ""),
                    "parameters": tool["input_schema"]
                }
            })
        return groq_tools

    def _completion_from(self, completion: ChatCompletion) -> common.Completion:
        content_blocks = []

        message = completion.choices[0].message

        if message.content:
            content_blocks.append(common.TextRaw(message.content))

        if message.tool_calls:
            for tool_call in message.tool_calls:
                # parse arguments if they're a string
                args = tool_call.function.arguments
                if isinstance(args, str):
                    try:
                        args = json.loads(args)
                    except json.JSONDecodeError:
                        args = {"raw": args}

                content_blocks.append(common.ToolUse(
                    name=tool_call.function.name,
                    input=args,
                    id=tool_call.id
                ))

        # map Groq stop reasons to common format
        stop_reason_map = {
            "stop": "end_turn",
            "length": "max_tokens",
            "tool_calls": "tool_use",
        }

        groq_stop_reason = completion.choices[0].finish_reason
        stop_reason = cast(Literal["end_turn", "max_tokens", "stop_sequence", "tool_use", "unknown"],
                          stop_reason_map.get(groq_stop_reason, "unknown"))

        return common.Completion(
            role="assistant",
            content=content_blocks,
            input_tokens=completion.usage.prompt_tokens if completion.usage else 0,
            output_tokens=completion.usage.completion_tokens if completion.usage else 0,
            stop_reason=stop_reason,
        )

    @retry_rate_limits
    async def completion(
        self,
        messages: List[common.Message],
        max_tokens: int,
        model: str | None = None,
        temperature: float = 1.0,
        tools: List[common.Tool] | None = None,
        tool_choice: str | None = None,
        system_prompt: str | None = None,
        *args,
        **kwargs,
    ) -> common.Completion:
        chosen_model = model or self.default_model
        groq_messages = self._messages_into(messages)

        if system_prompt:
            groq_messages.insert(0, {"role": "system", "content": system_prompt})

        call_args: GroqParams = {
            "messages": groq_messages,
            "model": chosen_model,
            "temperature": temperature,
            "max_tokens": max_tokens,
        }

        groq_tools = self._tools_into(tools)
        if groq_tools:
            call_args["tools"] = groq_tools
            logger.debug(f"Tools provided: {groq_tools}")

        if tool_choice:
            call_args["tool_choice"] = cast(Any, {"type": "function", "function": {"name": tool_choice}})

        try:
            telemetry = LLMTelemetry()
            telemetry.start_timing()

            logger.debug(f"Calling Groq API with messages: {call_args['messages']}")
            completion = await self.client.chat.completions.create(**cast(Any, call_args))


            # log telemetry if usage data is available
            if completion.usage:
                telemetry.log_completion(
                    model=chosen_model,
                    input_tokens=completion.usage.prompt_tokens,
                    output_tokens=completion.usage.completion_tokens,
                    temperature=temperature,
                    has_tools=groq_tools is not None,
                    provider="Groq"
                )

            logger.debug(f"Groq API response: {completion}")
            return self._completion_from(completion)
        except groq.BadRequestError as e:
            error_message = str(e)
            logger.error(f"Groq BadRequestError: {error_message}")
            
            # Handle tool_use_failed errors
            if "tool_use_failed" in error_message:
                logger.warning("Model generated malformed tool calls. This is a known issue with some prompts.")
                # Try to extract any failed generation info
                if hasattr(e, 'response') and hasattr(e.response, 'json'):
                    try:
                        error_data = e.response.json()
                        if 'error' in error_data and 'failed_generation' in error_data['error']:
                            logger.error(f"Failed generation: {error_data['error']['failed_generation']}")
                    except:
                        pass
                
                # Return a helpful error message
                return common.Completion(
                    role="assistant",
                    content=[common.TextRaw(
                        "I encountered an error while trying to use the requested tools. "
                        "The model had difficulty generating proper tool calls. "
                        "Please try rephrasing your request or simplifying the task."
                    )],
                    input_tokens=0,
                    output_tokens=0,
                    stop_reason="end_turn"
                )
            raise
        except Exception as e:
            logger.error(f"Groq API error: {e}")
            raise


if __name__ == "__main__":
    # Example usage
    import asyncio

    async def main():
        llm = GroqLLM()
        messages = [
            common.Message(role="user", content=[common.TextRaw("Hello, how are you?")])
        ]
        completion = await llm.completion(
            messages=messages,
            max_tokens=50,
            model="moonshotai/kimi-k2-instruct",
            temperature=0.7
        )
        print("Completion:", completion)

    asyncio.run(main())

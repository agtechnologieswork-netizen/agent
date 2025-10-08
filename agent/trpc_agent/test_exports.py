import os
import anyio
import dagger
from core.dagger_utils import export_application_artifact

async def main():
    async with dagger.Connection(
        dagger.Config(log_output=open(os.devnull, "w"))
    ) as client:
        await export_application_artifact({}, "bundle_server", client)


if __name__ == "__main__":
    anyio.run(main)

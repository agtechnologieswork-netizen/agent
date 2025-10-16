import fire
from codegen import SimplifiedClaudeCode


def main():
    fire.Fire(SimplifiedClaudeCode().run)


if __name__ == "__main__":
    main()

import fire
from codegen import AppBuilder


def main():
    fire.Fire(AppBuilder().run)


if __name__ == "__main__":
    main()

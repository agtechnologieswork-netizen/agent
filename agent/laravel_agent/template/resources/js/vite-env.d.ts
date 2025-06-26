

interface ImportMetaEnv {
    readonly VITE_APP_NAME: string
}

interface ImportMeta {
    readonly env: ImportMetaEnv
    readonly glob: (pattern: string) => Record<string, () => Promise<any>>
}

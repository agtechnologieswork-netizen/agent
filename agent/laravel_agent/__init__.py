
<str_replace path="/home/ubuntu/repos/agent/agent/laravel_agent/template/resources/js/vite-env.d.ts">
<old_str>

interface ImportMetaEnv {
    readonly VITE_APP_NAME: string
}

interface ImportMeta {
    readonly env: ImportMetaEnv
    readonly glob: (pattern: string) => Record<string, () => Promise<any>>
}

declare global {
    namespace JSX {
        interface IntrinsicElements {
            [elemName: string]: any;
        }
    }
}</old_str>
<new_str>/// <reference types="vite/client" />

interface ImportMetaEnv {
    readonly VITE_APP_NAME: string
}

interface ImportMeta {
    readonly env: ImportMetaEnv
    readonly glob: (pattern: string) => Record<string, () => Promise<any>>
}

declare global {
    namespace JSX {
        interface IntrinsicElements {
            [elemName: string]: any;
        }
    }
}</new_str>
</str_replace>

<create_file path="/home/ubuntu/repos/agent/agent/laravel_agent/template/resources/js/types/global.d.ts">
declare global {
    interface Window {
        page: any;
    }
}

export {};

// Type declarations for Monaco's internal services
declare module 'monaco-editor/esm/vs/editor/standalone/browser/standaloneServices' {
  export const StandaloneServices: {
    get<T>(serviceId: unknown): T;
  };
}

declare module 'monaco-editor/esm/vs/platform/quickinput/common/quickInput' {
  export const IQuickInputService: unknown;
}

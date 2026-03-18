import type { LocalizedText } from './i18n';

export type WorkspaceId =
  | 'command-center'
  | 'traffic-lab'
  | 'provider-atlas'
  | 'route-studio'
  | 'change-studio';

export type SourceMode = 'runtime' | 'hybrid' | 'external';
export type LocaleMode = 'en-US' | 'zh-CN' | 'en-XA';
export type EnvironmentMode = 'production' | 'staging';
export type TimeRangeMode = '15m' | '1h' | '6h' | '24h';

export interface ShellInspectorSection {
  title: LocalizedText;
  rows: Array<{ label: LocalizedText; value: string; value_text?: LocalizedText }>;
}

export interface ShellInspectorAction {
  id: string;
  label: LocalizedText;
  effect: 'navigate' | 'reload' | 'invoke';
  target_workspace?: WorkspaceId;
}

export interface ShellInspectorState {
  eyebrow: LocalizedText;
  title: LocalizedText;
  summary: LocalizedText;
  sections: ShellInspectorSection[];
  actions: ShellInspectorAction[];
}

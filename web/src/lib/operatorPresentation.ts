import type { LocalizedPrimitive, LocalizedText } from '../types/i18n';

type Translate = (key: string, values?: Record<string, LocalizedPrimitive>) => string;

function humanize(value: string) {
  return value
    .replace(/[_-]+/g, ' ')
    .replace(/([a-z0-9])([A-Z])/g, '$1 $2')
    .split(' ')
    .filter(Boolean)
    .map((part) => {
      const lower = part.toLowerCase();
      if (lower === 'api') return 'API';
      if (lower === 'json') return 'JSON';
      if (lower === 'oauth') return 'OAuth';
      if (lower === 'id') return 'ID';
      if (lower === 'claude') return 'Claude';
      if (lower === 'gemini') return 'Gemini';
      if (lower === 'codex') return 'Codex';
      if (lower === 'openai') return 'OpenAI';
      return `${lower.slice(0, 1).toUpperCase()}${lower.slice(1)}`;
    })
    .join(' ');
}

export function presentFactValue(
  fact: { value: string; value_text?: LocalizedText | null },
  tx: (text: LocalizedText) => string,
) {
  return fact.value_text ? tx(fact.value_text) : fact.value;
}

export function presentProviderFormat(format: string) {
  const normalized = format.trim().toLowerCase();
  if (!normalized) return format;
  if (normalized === 'openai') return 'OpenAI';
  if (normalized === 'claude') return 'Claude';
  if (normalized === 'gemini') return 'Gemini';
  return humanize(format);
}

export function presentProbeStatus(status: string, t: Translate) {
  const normalized = status.trim().toLowerCase();
  switch (normalized) {
    case 'ok':
      return t('providerAtlas.value.probe.ok');
    case 'warning':
      return t('providerAtlas.value.probe.warning');
    case 'error':
      return t('providerAtlas.value.probe.error');
    case 'verified':
      return t('providerAtlas.value.probe.verified');
    case 'failed':
      return t('providerAtlas.value.probe.failed');
    case 'unknown':
      return t('providerAtlas.value.probe.unknown');
    case 'unsupported':
      return t('providerAtlas.value.probe.unsupported');
    default:
      return humanize(status);
  }
}

export function presentCapabilityName(capability: string, t: Translate) {
  const normalized = capability.trim().toLowerCase();
  switch (normalized) {
    case 'auth':
      return t('providerAtlas.value.capability.auth');
    case 'text':
      return t('providerAtlas.value.capability.text');
    case 'stream':
      return t('providerAtlas.value.capability.stream');
    case 'tools':
      return t('providerAtlas.value.capability.tools');
    case 'images':
      return t('providerAtlas.value.capability.images');
    case 'json_schema':
      return t('providerAtlas.value.capability.jsonSchema');
    case 'reasoning':
      return t('providerAtlas.value.capability.reasoning');
    case 'count_tokens':
      return t('providerAtlas.value.capability.countTokens');
    default:
      return humanize(capability);
  }
}

export function presentAuthMode(mode: string, t: Translate) {
  const normalized = mode.trim().toLowerCase();
  switch (normalized) {
    case 'api-key':
      return t('providerAtlas.auth.apiKey');
    case 'bearer-token':
      return t('providerAtlas.auth.bearerToken');
    case 'codex-oauth':
      return t('providerAtlas.auth.codexOauth');
    case 'anthropic-claude-subscription':
      return t('providerAtlas.auth.claudeSubscription');
    default:
      return humanize(mode);
  }
}

export function presentWireApi(wireApi: string, t: Translate) {
  const normalized = wireApi.trim().toLowerCase();
  switch (normalized) {
    case 'chat':
      return t('providerAtlas.value.wire.chat');
    case 'responses':
      return t('providerAtlas.value.wire.responses');
    default:
      return humanize(wireApi);
  }
}

export function presentExecutionMode(mode: string | null | undefined, t: Translate) {
  const normalized = mode?.trim().toLowerCase();
  switch (normalized) {
    case 'native':
      return t('providerAtlas.value.execution.native');
    case 'lossless_adapted':
      return t('providerAtlas.value.execution.losslessAdapted');
    case 'lossy_adapted':
      return t('providerAtlas.value.execution.lossyAdapted');
    default:
      return t('providerAtlas.protocol.unsupported');
  }
}

export function presentPresentationProfile(profile: string, t: Translate) {
  const normalized = profile.trim().toLowerCase();
  switch (normalized) {
    case '':
      return t('common.notConfigured');
    case 'native':
      return t('providerAtlas.value.presentation.native');
    case 'claude-code':
    case 'claude_code':
      return t('providerAtlas.value.presentation.claudeCode');
    case 'gemini-cli':
    case 'gemini_cli':
      return t('providerAtlas.value.presentation.geminiCli');
    case 'codex-cli':
    case 'codex_cli':
      return t('providerAtlas.value.presentation.codexCli');
    default:
      return humanize(profile);
  }
}

export function presentPresentationMode(mode: string, t: Translate) {
  const normalized = mode.trim().toLowerCase();
  switch (normalized) {
    case 'always':
      return t('providerAtlas.value.presentationMode.always');
    case 'auto':
      return t('providerAtlas.value.presentationMode.auto');
    default:
      return humanize(mode);
  }
}

export function presentMutationKind(kind: string, t: Translate) {
  const normalized = kind.trim().toLowerCase();
  switch (normalized) {
    case 'system_prompt_injection':
      return t('providerAtlas.value.mutation.systemPromptInjection');
    case 'user_id_generation':
      return t('providerAtlas.value.mutation.userIdGeneration');
    case 'sensitive_word_obfuscation':
      return t('providerAtlas.value.mutation.sensitiveWordObfuscation');
    default:
      return humanize(kind);
  }
}

export function presentScopeValue(value: string, t: Translate) {
  const normalized = value.trim().toLowerCase();
  if (normalized === 'global') return t('common.global');
  if (normalized === 'unscoped') return t('common.unscoped');
  return value;
}

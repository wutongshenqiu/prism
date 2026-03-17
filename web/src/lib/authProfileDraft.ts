import type { CodexDeviceStartResponse } from '../types/backend';

export interface AuthProfileFormState {
  provider: string;
  id: string;
  mode: string;
  secret: string;
  disabled: boolean;
  weight: string;
  region: string;
  prefix: string;
}

export interface DeviceFlowState extends CodexDeviceStartResponse {
  status: 'pending';
  target_profile_key: string;
  target_qualified_name: string;
}

export const emptyProfileForm: AuthProfileFormState = {
  provider: '',
  id: 'primary',
  mode: 'api-key',
  secret: '',
  disabled: false,
  weight: '1',
  region: '',
  prefix: '',
};

export function profileKey(provider: string, profileId: string) {
  return `${provider}/${profileId}`;
}

export function isManagedMode(mode: string) {
  return mode === 'codex-oauth' || mode === 'anthropic-claude-subscription';
}

export function resolveDeviceFlowProfileLabel(
  deviceFlow: Pick<DeviceFlowState, 'target_qualified_name'>,
  profile?: { qualified_name?: string | null } | null,
) {
  return profile?.qualified_name ?? deviceFlow.target_qualified_name;
}

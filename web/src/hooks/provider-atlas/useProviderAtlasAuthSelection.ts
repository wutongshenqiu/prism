import {
  useCallback,
  useEffect,
  useMemo,
  useState,
  type Dispatch,
  type SetStateAction,
} from 'react';
import {
  emptyProfileForm,
  profileKey,
  type AuthProfileFormState,
  type DeviceFlowState,
} from '../../lib/authProfileDraft';
import { reconcileSelection } from '../../lib/selection';
import { authProfilesApi } from '../../services/authProfiles';
import type {
  AuthProfileSummary,
  AuthProfilesRuntimeResponse,
} from '../../types/backend';
import type { ProviderAtlasResponse } from '../../types/controlPlane';

interface UseProviderAtlasAuthSelectionOptions {
  providers: ProviderAtlasResponse['providers'];
  selectedProvider: string | null;
  setRuntimeInfo: Dispatch<SetStateAction<AuthProfilesRuntimeResponse | null>>;
}

export function useProviderAtlasAuthSelection({
  providers,
  selectedProvider,
  setRuntimeInfo,
}: UseProviderAtlasAuthSelectionOptions) {
  const [authWorkbenchOpen, setAuthWorkbenchOpen] = useState(false);
  const [profiles, setProfiles] = useState<AuthProfileSummary[]>([]);
  const [selectedAuthProfileId, setSelectedAuthProfileId] = useState<string | null>(null);
  const [authForm, setAuthForm] = useState<AuthProfileFormState>(emptyProfileForm);
  const [authLoading, setAuthLoading] = useState(false);
  const [authEditorMode, setAuthEditorMode] = useState<'create' | 'edit'>('create');
  const [connectSecret, setConnectSecret] = useState('');
  const [importPath, setImportPath] = useState('');
  const [deviceFlow, setDeviceFlow] = useState<DeviceFlowState | null>(null);

  const loadProfiles = useCallback(async () => {
    const [runtime, profileList] = await Promise.all([
      authProfilesApi.runtime(),
      authProfilesApi.list(),
    ]);
    setRuntimeInfo(runtime);
    setProfiles(profileList.profiles);
    return profileList.profiles;
  }, [setRuntimeInfo]);

  useEffect(() => {
    let active = true;

    void loadProfiles().catch(() => {
      if (!active) {
        return;
      }
      setProfiles([]);
    });

    return () => {
      active = false;
    };
  }, [loadProfiles]);

  useEffect(() => {
    setAuthForm((current) => {
      const provider = current.provider || selectedProvider || providers[0]?.provider || '';
      return provider === current.provider ? current : { ...current, provider };
    });
  }, [providers, selectedProvider]);

  const selectedProfiles = useMemo(
    () => profiles.filter((profile) => profile.provider === (authForm.provider || selectedProvider)),
    [authForm.provider, profiles, selectedProvider],
  );
  const selectedAuthProfile = useMemo(
    () =>
      selectedProfiles.find((profile) => profileKey(profile.provider, profile.id) === selectedAuthProfileId) ??
      null,
    [selectedAuthProfileId, selectedProfiles],
  );
  const selectedProviderName = authForm.provider || selectedProvider || providers[0]?.provider || '';
  const selectedAuthProfileMode = selectedAuthProfile?.mode ?? authForm.mode;

  useEffect(() => {
    setSelectedAuthProfileId((current) =>
      reconcileSelection(current, selectedProfiles, (profile) =>
        profileKey(profile.provider, profile.id),
      ),
    );
  }, [selectedProfiles]);

  useEffect(() => {
    if (!selectedAuthProfile) {
      return;
    }
    setAuthEditorMode('edit');
    setAuthForm({
      provider: selectedAuthProfile.provider,
      id: selectedAuthProfile.id,
      mode: selectedAuthProfile.mode,
      secret: '',
      disabled: selectedAuthProfile.disabled,
      weight: String(selectedAuthProfile.weight ?? 1),
      region: selectedAuthProfile.region ?? '',
      prefix: selectedAuthProfile.prefix ?? '',
    });
    setConnectSecret('');
  }, [selectedAuthProfile]);

  const openAuthWorkbench = useCallback(async () => {
    setAuthWorkbenchOpen(true);
    setAuthEditorMode('create');
    setConnectSecret('');
    setImportPath('');
    setDeviceFlow(null);
    setSelectedAuthProfileId(null);
    setAuthLoading(true);
    try {
      const profileList = await loadProfiles();
      const preferredProvider =
        selectedProvider ?? profileList[0]?.provider ?? providers[0]?.provider ?? '';
      const preferredProfile =
        profileList.find((profile) => profile.provider === preferredProvider) ??
        profileList[0] ??
        null;
      setSelectedAuthProfileId(
        preferredProfile ? profileKey(preferredProfile.provider, preferredProfile.id) : null,
      );
      setAuthForm({
        ...emptyProfileForm,
        provider: preferredProvider,
      });
    } finally {
      setAuthLoading(false);
    }
  }, [loadProfiles, providers, selectedProvider]);

  const startNewAuthProfileDraft = useCallback(() => {
    setAuthEditorMode('create');
    setSelectedAuthProfileId(null);
    setConnectSecret('');
    setImportPath('');
    setDeviceFlow(null);
    setAuthForm({
      ...emptyProfileForm,
      provider: selectedProviderName,
    });
  }, [selectedProviderName]);

  return {
    authWorkbenchOpen,
    setAuthWorkbenchOpen,
    authLoading,
    authEditorMode,
    authForm,
    setAuthForm,
    profiles,
    selectedProfiles,
    selectedAuthProfile,
    selectedAuthProfileId,
    setSelectedAuthProfileId,
    selectedAuthProfileMode,
    selectedProviderName,
    connectSecret,
    setConnectSecret,
    importPath,
    setImportPath,
    deviceFlow,
    setDeviceFlow,
    loadProfiles,
    openAuthWorkbench,
    startNewAuthProfileDraft,
  };
}

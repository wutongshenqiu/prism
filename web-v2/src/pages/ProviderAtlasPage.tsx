import { useEffect, useMemo, useState } from 'react';
import { ProviderAtlasOverview } from '../components/provider-atlas/ProviderAtlasOverview';
import {
  AuthProfileWorkbenchSheet,
  ProviderEditorSheet,
  ProviderRegistrySheet,
} from '../components/provider-atlas/ProviderAtlasSheets';
import {
  type ProviderEditorFormState,
  type ProviderRegistryFormState,
} from '../components/provider-atlas/types';
import { useProviderAtlasData } from '../hooks/useWorkspaceData';
import {
  emptyProfileForm,
  isManagedMode,
  profileKey,
  type AuthProfileFormState,
  type DeviceFlowState,
} from '../lib/authProfileDraft';
import { authProfilesApi } from '../services/authProfiles';
import { getApiErrorMessage } from '../services/errors';
import { protocolsApi } from '../services/protocols';
import { providersApi } from '../services/providers';
import type {
  AuthProfileSummary,
  AuthProfilesRuntimeResponse,
  PresentationPreviewResponse,
  ProtocolMatrixResponse,
  ProviderCapabilityEntry,
  ProviderCreateRequest,
  ProviderDetail,
  ProviderHealthResult,
} from '../types/backend';

const emptyRegistryForm: ProviderRegistryFormState = {
  name: '',
  format: 'openai',
  upstream: 'openai',
  apiKey: '',
  baseUrl: '',
  models: '',
  disabled: true,
};

export function ProviderAtlasPage() {
  const { data, error, loading, reload } = useProviderAtlasData();
  const [selectedProvider, setSelectedProvider] = useState<string | null>(null);
  const [editorOpen, setEditorOpen] = useState(false);
  const [registryOpen, setRegistryOpen] = useState(false);
  const [authWorkbenchOpen, setAuthWorkbenchOpen] = useState(false);
  const [detail, setDetail] = useState<ProviderDetail | null>(null);
  const [health, setHealth] = useState<ProviderHealthResult | null>(null);
  const [preview, setPreview] = useState<PresentationPreviewResponse | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [actionStatus, setActionStatus] = useState<string | null>(null);
  const [loadingDetail, setLoadingDetail] = useState(false);
  const [saving, setSaving] = useState(false);
  const [previewing, setPreviewing] = useState(false);
  const [runtimeInfo, setRuntimeInfo] = useState<AuthProfilesRuntimeResponse | null>(null);
  const [capabilityEntries, setCapabilityEntries] = useState<ProviderCapabilityEntry[]>([]);
  const [protocolMatrix, setProtocolMatrix] = useState<ProtocolMatrixResponse | null>(null);
  const [profiles, setProfiles] = useState<AuthProfileSummary[]>([]);
  const [refreshingProfileId, setRefreshingProfileId] = useState<string | null>(null);
  const [selectedAuthProfileId, setSelectedAuthProfileId] = useState<string | null>(null);
  const [importingProfileId, setImportingProfileId] = useState<string | null>(null);
  const [formState, setFormState] = useState<ProviderEditorFormState>({
    baseUrl: '',
    region: '',
    weight: '1',
    disabled: false,
  });
  const [registryForm, setRegistryForm] = useState<ProviderRegistryFormState>(emptyRegistryForm);
  const [registryLoading, setRegistryLoading] = useState(false);
  const [registryStatus, setRegistryStatus] = useState<string | null>(null);
  const [registryError, setRegistryError] = useState<string | null>(null);
  const [authForm, setAuthForm] = useState<AuthProfileFormState>(emptyProfileForm);
  const [authStatus, setAuthStatus] = useState<string | null>(null);
  const [authError, setAuthError] = useState<string | null>(null);
  const [authLoading, setAuthLoading] = useState(false);
  const [authSaving, setAuthSaving] = useState(false);
  const [authEditorMode, setAuthEditorMode] = useState<'create' | 'edit'>('create');
  const [connectSecret, setConnectSecret] = useState('');
  const [importPath, setImportPath] = useState('');
  const [connectingProfileId, setConnectingProfileId] = useState<string | null>(null);
  const [deviceFlow, setDeviceFlow] = useState<DeviceFlowState | null>(null);
  const [protocolSearch, setProtocolSearch] = useState('');
  const [modelSearch, setModelSearch] = useState('');

  useEffect(() => {
    setSelectedProvider((current) => current ?? data?.providers[0]?.provider ?? null);
  }, [data]);

  const loadRuntimeSurfaces = async () => {
    const [capabilities, protocols, profileList] = await Promise.all([
      providersApi.capabilities(),
      protocolsApi.matrix(),
      authProfilesApi.list(),
    ]);
    setCapabilityEntries(capabilities.providers);
    setProtocolMatrix(protocols);
    setProfiles(profileList.profiles);
  };

  useEffect(() => {
    let active = true;

    void (async () => {
      try {
        const [capabilities, protocols, profileList] = await Promise.all([
          providersApi.capabilities(),
          protocolsApi.matrix(),
          authProfilesApi.list(),
        ]);
        if (!active) {
          return;
        }
        setCapabilityEntries(capabilities.providers);
        setProtocolMatrix(protocols);
        setProfiles(profileList.profiles);
      } catch {
        if (!active) {
          return;
        }
        setCapabilityEntries([]);
        setProtocolMatrix(null);
        setProfiles([]);
      }
    })();

    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    setAuthForm((current) => ({
      ...current,
      provider: current.provider || selectedProvider || data?.providers[0]?.provider || '',
    }));
  }, [data?.providers, selectedProvider]);

  const selectedRow = useMemo(
    () => data?.providers.find((provider) => provider.provider === selectedProvider) ?? null,
    [data, selectedProvider],
  );
  const selectedCapabilities = useMemo(
    () => capabilityEntries.find((provider) => provider.name === selectedProvider) ?? null,
    [capabilityEntries, selectedProvider],
  );
  const selectedProfiles = useMemo(
    () => profiles.filter((profile) => profile.provider === (authForm.provider || selectedProvider)),
    [authForm.provider, profiles, selectedProvider],
  );
  const selectedAuthProfile = useMemo(
    () => selectedProfiles.find((profile) => profileKey(profile.provider, profile.id) === selectedAuthProfileId) ?? null,
    [selectedAuthProfileId, selectedProfiles],
  );
  const selectedProviderName = authForm.provider || selectedProvider || data?.providers[0]?.provider || '';
  const selectedAuthProfileMode = selectedAuthProfile?.mode ?? authForm.mode;
  const protocolFacts = useMemo(() => {
    const endpoints = protocolMatrix?.endpoints ?? [];
    const coverage = protocolMatrix?.coverage.filter((entry) => !entry.disabled) ?? [];
    return {
      publicRoutes: endpoints.filter((entry) => entry.scope === 'public').length,
      providerRoutes: endpoints.filter((entry) => entry.scope === 'provider_scoped').length,
      nativeSurfaces: coverage.filter((entry) => entry.execution_mode === 'native').length,
      adaptedSurfaces: coverage.filter((entry) => entry.execution_mode && entry.execution_mode !== 'native').length,
    };
  }, [protocolMatrix]);
  const modelInventory = useMemo(() => {
    const rows = capabilityEntries
      .filter((entry) => !entry.disabled)
      .flatMap((entry) =>
        entry.models.map((model) => ({
          id: model.alias ?? model.id,
          provider: entry.name,
          upstream: entry.upstream,
          probe: entry.probe_status,
        })),
      );
    return rows.slice(0, 8);
  }, [capabilityEntries]);
  const filteredProtocolCoverage = useMemo(() => {
    const needle = protocolSearch.trim().toLowerCase();
    return (protocolMatrix?.coverage ?? [])
      .filter((entry) => entry.provider === selectedProvider)
      .filter((entry) => {
        if (!needle) {
          return true;
        }
        return [
          entry.surface_label,
          entry.surface_id,
          entry.execution_mode ?? '',
          entry.upstream,
        ].join(' ').toLowerCase().includes(needle);
      })
      .slice(0, 8);
  }, [protocolMatrix?.coverage, protocolSearch, selectedProvider]);
  const filteredModelInventory = useMemo(() => {
    const needle = modelSearch.trim().toLowerCase();
    return modelInventory.filter((item) => {
      if (!needle) {
        return true;
      }
      return [item.id, item.provider, item.upstream, item.probe].join(' ').toLowerCase().includes(needle);
    });
  }, [modelInventory, modelSearch]);

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

  const openEditor = async () => {
    if (!selectedProvider) {
      return;
    }
    setEditorOpen(true);
    setLoadingDetail(true);
    setActionError(null);
    setActionStatus(null);
    setHealth(null);
    setPreview(null);

    try {
      const [provider, runtime] = await Promise.all([
        providersApi.get(selectedProvider),
        authProfilesApi.runtime(),
      ]);
      setDetail(provider);
      setRuntimeInfo(runtime);
      setFormState({
        baseUrl: provider.base_url ?? '',
        region: provider.region ?? '',
        weight: String(provider.weight ?? 1),
        disabled: provider.disabled,
      });
    } catch (editorError) {
      setActionError(getApiErrorMessage(editorError, 'Failed to load provider detail'));
    } finally {
      setLoadingDetail(false);
    }
  };

  const openRegistryWorkbench = () => {
    setRegistryOpen(true);
    setRegistryStatus(null);
    setRegistryError(null);
    setRegistryForm(emptyRegistryForm);
  };

  const openAuthWorkbench = async () => {
    setAuthWorkbenchOpen(true);
    setAuthStatus(null);
    setAuthError(null);
    setAuthEditorMode('create');
    setConnectSecret('');
    setImportPath('');
    setDeviceFlow(null);
    setSelectedAuthProfileId(null);
    setAuthLoading(true);
    try {
      const [runtime, profileList] = await Promise.all([
        authProfilesApi.runtime(),
        authProfilesApi.list(),
      ]);
      setRuntimeInfo(runtime);
      setProfiles(profileList.profiles);
      const preferredProvider = selectedProvider ?? profileList.profiles[0]?.provider ?? data?.providers[0]?.provider ?? '';
      const preferredProfile = profileList.profiles.find((profile) => profile.provider === preferredProvider) ?? profileList.profiles[0] ?? null;
      setSelectedAuthProfileId(preferredProfile ? profileKey(preferredProfile.provider, preferredProfile.id) : null);
      setAuthForm({
        ...emptyProfileForm,
        provider: preferredProvider,
      });
    } catch (loadError) {
      setAuthError(getApiErrorMessage(loadError, 'Failed to load auth profiles'));
    } finally {
      setAuthLoading(false);
    }
  };

  const refreshAuthProfile = async (provider: string, profileId: string) => {
    setRefreshingProfileId(profileKey(provider, profileId));
    setAuthError(null);
    setAuthStatus(null);
    try {
      const response = await authProfilesApi.refresh(provider, profileId);
      setAuthStatus(`Refreshed auth profile ${response.profile.qualified_name}.`);
      await loadRuntimeSurfaces();
      if (selectedProvider === provider) {
        const refreshed = await providersApi.get(provider);
        setDetail(refreshed);
      }
    } catch (refreshError) {
      setAuthError(getApiErrorMessage(refreshError, 'Failed to refresh auth profile'));
    } finally {
      setRefreshingProfileId(null);
    }
  };

  const importSelectedProfile = async () => {
    if (!selectedAuthProfile) {
      setAuthError('Select an auth profile first.');
      return;
    }

    setImportingProfileId(profileKey(selectedAuthProfile.provider, selectedAuthProfile.id));
    setAuthError(null);
    setAuthStatus(null);
    try {
      const response = await authProfilesApi.importLocal(
        selectedAuthProfile.provider,
        selectedAuthProfile.id,
        importPath.trim() || undefined,
      );
      setAuthStatus(`Imported local credentials into ${response.profile.qualified_name}.`);
      await loadRuntimeSurfaces();
    } catch (importError) {
      setAuthError(getApiErrorMessage(importError, 'Failed to import local auth state'));
    } finally {
      setImportingProfileId(null);
    }
  };

  const deleteSelectedProfile = async () => {
    if (!selectedAuthProfile) {
      setAuthError('Select an auth profile first.');
      return;
    }
    if (!window.confirm(`Delete auth profile "${selectedAuthProfile.qualified_name}"?`)) {
      return;
    }

    setAuthError(null);
    setAuthStatus(null);
    try {
      await authProfilesApi.remove(selectedAuthProfile.provider, selectedAuthProfile.id);
      setAuthStatus(`Deleted auth profile ${selectedAuthProfile.qualified_name}.`);
      setSelectedAuthProfileId(null);
      await loadRuntimeSurfaces();
      await reload();
    } catch (deleteError) {
      setAuthError(getApiErrorMessage(deleteError, 'Failed to delete auth profile'));
    }
  };

  const startNewAuthProfileDraft = () => {
    setAuthEditorMode('create');
    setSelectedAuthProfileId(null);
    setConnectSecret('');
    setImportPath('');
    setDeviceFlow(null);
    setAuthError(null);
    setAuthStatus(null);
    setAuthForm({
      ...emptyProfileForm,
      provider: selectedProviderName,
    });
  };

  const saveAuthProfile = async () => {
    if (!authForm.provider.trim() || !authForm.id.trim()) {
      setAuthError('Provider and profile id are required.');
      return;
    }

    if (!isManagedMode(authForm.mode) && authEditorMode === 'create' && !authForm.secret.trim()) {
      setAuthError('Secret is required for API key and bearer token auth profiles.');
      return;
    }

    setAuthSaving(true);
    setAuthError(null);
    setAuthStatus(null);
    try {
      const payload = {
        mode: authForm.mode,
        secret: isManagedMode(authForm.mode) ? undefined : authForm.secret.trim() || undefined,
        disabled: authForm.disabled,
        weight: Number(authForm.weight) || 1,
        region: authForm.region.trim() || null,
        prefix: authForm.prefix.trim() || null,
      };

      const response = authEditorMode === 'edit' && selectedAuthProfile
        ? await authProfilesApi.replace(selectedAuthProfile.provider, selectedAuthProfile.id, payload)
        : await authProfilesApi.create({
            provider: authForm.provider.trim(),
            id: authForm.id.trim(),
            ...payload,
          });

      setAuthStatus(`${authEditorMode === 'edit' ? 'Saved' : 'Created'} auth profile ${response.profile.qualified_name}.`);
      setSelectedAuthProfileId(profileKey(response.profile.provider, response.profile.id));
      setAuthForm((current) => ({ ...current, secret: '' }));
      await loadRuntimeSurfaces();
      await reload();
    } catch (createError) {
      setAuthError(getApiErrorMessage(createError, 'Failed to save auth profile'));
    } finally {
      setAuthSaving(false);
    }
  };

  const connectSelectedProfile = async () => {
    if (!selectedAuthProfile) {
      setAuthError('Select an auth profile first.');
      return;
    }
    if (selectedAuthProfile.mode !== 'anthropic-claude-subscription') {
      setAuthError('Secret connect is only supported for Claude subscription profiles.');
      return;
    }
    if (!connectSecret.trim()) {
      setAuthError('Enter the subscription token first.');
      return;
    }

    const currentKey = profileKey(selectedAuthProfile.provider, selectedAuthProfile.id);
    setConnectingProfileId(currentKey);
    setAuthError(null);
    setAuthStatus(null);
    try {
      const response = await authProfilesApi.connect(selectedAuthProfile.provider, selectedAuthProfile.id, {
        secret: connectSecret.trim(),
      });
      setAuthStatus(`Connected ${response.profile.qualified_name}.`);
      setConnectSecret('');
      await loadRuntimeSurfaces();
      await reload();
    } catch (connectError) {
      setAuthError(getApiErrorMessage(connectError, 'Failed to connect auth profile'));
    } finally {
      setConnectingProfileId(null);
    }
  };

  const startBrowserOauth = async () => {
    if (!selectedAuthProfile) {
      setAuthError('Select an auth profile first.');
      return;
    }
    if (selectedAuthProfile.mode !== 'codex-oauth') {
      setAuthError('Browser OAuth is only available for Codex OAuth profiles.');
      return;
    }

    const currentKey = profileKey(selectedAuthProfile.provider, selectedAuthProfile.id);
    setConnectingProfileId(currentKey);
    setAuthError(null);
    setAuthStatus(null);
    try {
      const redirectUri = `${window.location.origin}/provider-atlas/callback`;
      const response = await authProfilesApi.startCodexOauth({
        provider: selectedAuthProfile.provider,
        profile_id: selectedAuthProfile.id,
        redirect_uri: redirectUri,
      });
      window.location.assign(response.auth_url);
    } catch (startError) {
      setAuthError(getApiErrorMessage(startError, 'Failed to start browser OAuth'));
      setConnectingProfileId(null);
    }
  };

  const startDeviceFlow = async () => {
    if (!selectedAuthProfile) {
      setAuthError('Select an auth profile first.');
      return;
    }
    if (selectedAuthProfile.mode !== 'codex-oauth') {
      setAuthError('Device flow is only available for Codex OAuth profiles.');
      return;
    }

    const currentKey = profileKey(selectedAuthProfile.provider, selectedAuthProfile.id);
    setConnectingProfileId(currentKey);
    setAuthError(null);
    setAuthStatus(null);
    try {
      const response = await authProfilesApi.startCodexDevice({
        provider: selectedAuthProfile.provider,
        profile_id: selectedAuthProfile.id,
      });
      setDeviceFlow({ ...response, status: 'pending' });
      setAuthStatus(`Started device flow for ${selectedAuthProfile.qualified_name}.`);
    } catch (startError) {
      setAuthError(getApiErrorMessage(startError, 'Failed to start device flow'));
    } finally {
      setConnectingProfileId(null);
    }
  };

  useEffect(() => {
    if (!selectedAuthProfile || !deviceFlow) {
      return;
    }

    let cancelled = false;
    const interval = window.setInterval(() => {
      if (cancelled) {
        return;
      }
      void authProfilesApi.pollCodexDevice(deviceFlow.state)
        .then(async (result) => {
          if (cancelled || result.status !== 'completed') {
            return;
          }
          setAuthStatus(`Connected ${selectedAuthProfile.qualified_name} via device flow.`);
          setDeviceFlow(null);
          await loadRuntimeSurfaces();
          await reload();
        })
        .catch((pollError) => {
          if (cancelled) {
            return;
          }
          setAuthError(getApiErrorMessage(pollError, 'Device flow polling failed'));
        });
    }, Math.max(deviceFlow.interval_secs, 2) * 1000);

    return () => {
      cancelled = true;
      window.clearInterval(interval);
    };
  }, [deviceFlow, reload, selectedAuthProfile]);

  const runHealthCheck = async () => {
    if (!selectedProvider) {
      return;
    }
    setActionError(null);
    setActionStatus('Running real provider health probe…');
    try {
      const result = await providersApi.healthCheck(selectedProvider);
      setHealth(result);
      setActionStatus(`Health probe completed with status ${result.status}.`);
    } catch (probeError) {
      setActionError(getApiErrorMessage(probeError, 'Health probe failed'));
    }
  };

  const runPresentationPreview = async () => {
    if (!selectedProvider) {
      return;
    }
    setPreviewing(true);
    setActionError(null);
    setActionStatus(null);
    try {
      const result = await providersApi.presentationPreview(selectedProvider, {
        model: detail?.models[0]?.id ?? selectedCapabilities?.models[0]?.id ?? 'gpt-5',
        user_agent: 'prism-control-plane-v2',
        sample_body: {
          input: 'hello',
          messages: [{ role: 'user', content: 'hello' }],
        },
      });
      setPreview(result);
      setActionStatus(`Presentation preview generated for ${selectedProvider}.`);
    } catch (previewError) {
      setActionError(getApiErrorMessage(previewError, 'Presentation preview failed'));
    } finally {
      setPreviewing(false);
    }
  };

  const saveProvider = async () => {
    if (!selectedProvider) {
      return;
    }
    setSaving(true);
    setActionError(null);
    setActionStatus(null);
    try {
      await providersApi.update(selectedProvider, {
        base_url: formState.baseUrl.trim() || null,
        region: formState.region.trim() || null,
        weight: Number(formState.weight) || 1,
        disabled: formState.disabled,
      });
      setActionStatus(`Saved provider ${selectedProvider}.`);
      await reload();
      await loadRuntimeSurfaces();
      const refreshed = await providersApi.get(selectedProvider);
      setDetail(refreshed);
    } catch (saveError) {
      setActionError(getApiErrorMessage(saveError, 'Failed to save provider'));
    } finally {
      setSaving(false);
    }
  };

  const fetchModelsIntoDraft = async () => {
    if (!registryForm.apiKey.trim()) {
      setRegistryError('An API key is required to fetch models.');
      return;
    }

    setRegistryLoading(true);
    setRegistryError(null);
    setRegistryStatus(null);
    try {
      const result = await providersApi.fetchModels({
        format: registryForm.format,
        upstream: registryForm.upstream,
        api_key: registryForm.apiKey.trim(),
        base_url: registryForm.baseUrl.trim() || undefined,
      });
      setRegistryForm((current) => ({ ...current, models: result.models.join(', ') }));
      setRegistryStatus(`Fetched ${result.models.length} models from upstream.`);
    } catch (fetchError) {
      setRegistryError(getApiErrorMessage(fetchError, 'Failed to fetch models'));
    } finally {
      setRegistryLoading(false);
    }
  };

  const createProvider = async () => {
    if (!registryForm.name.trim()) {
      setRegistryError('Provider name is required.');
      return;
    }

    const body: ProviderCreateRequest = {
      name: registryForm.name.trim(),
      format: registryForm.format,
      upstream: registryForm.upstream,
      api_key: registryForm.apiKey.trim() || undefined,
      base_url: registryForm.baseUrl.trim() || null,
      models: registryForm.models
        .split(',')
        .map((item) => item.trim())
        .filter(Boolean),
      disabled: registryForm.disabled,
    };

    setRegistryLoading(true);
    setRegistryError(null);
    setRegistryStatus(null);
    try {
      await providersApi.create(body);
      setRegistryStatus(`Created provider ${body.name}.`);
      setSelectedProvider(body.name);
      setRegistryForm(emptyRegistryForm);
      await reload();
      await loadRuntimeSurfaces();
    } catch (createError) {
      setRegistryError(getApiErrorMessage(createError, 'Failed to create provider'));
    } finally {
      setRegistryLoading(false);
    }
  };

  const deleteSelectedProvider = async () => {
    if (!selectedProvider) {
      setRegistryError('Select a provider first.');
      return;
    }
    if (!window.confirm(`Delete provider "${selectedProvider}"?`)) {
      return;
    }

    setRegistryLoading(true);
    setRegistryError(null);
    setRegistryStatus(null);
    try {
      await providersApi.remove(selectedProvider);
      setRegistryStatus(`Deleted provider ${selectedProvider}.`);
      setSelectedProvider(null);
      setDetail(null);
      await reload();
      await loadRuntimeSurfaces();
    } catch (deleteError) {
      setRegistryError(getApiErrorMessage(deleteError, 'Failed to delete provider'));
    } finally {
      setRegistryLoading(false);
    }
  };

  return (
    <div className="workspace-grid">
      <section className="hero">
        <div>
          <p className="workspace-eyebrow">PRISM / PROVIDER ATLAS</p>
          <h1>Runtime entities with identity and auth posture</h1>
          <p className="workspace-summary">
            Provider management should feel like runtime operations, not static CRUD. Coverage, auth state, protocol exposure, and routing participation stay visible together.
          </p>
        </div>
        <div className="hero-actions">
          <button className="button button--primary" onClick={() => void openEditor()}>
            Open provider editor
          </button>
          <button className="button button--ghost" onClick={() => void openAuthWorkbench()}>
            Auth profile workbench
          </button>
        </div>
      </section>

      <ProviderAtlasOverview
        loading={loading}
        error={error}
        data={data}
        selectedProvider={selectedProvider}
        selectedRow={selectedRow}
        selectedCapabilities={selectedCapabilities}
        protocolFacts={protocolFacts}
        filteredProtocolCoverage={filteredProtocolCoverage}
        filteredModelInventory={filteredModelInventory}
        protocolSearch={protocolSearch}
        modelSearch={modelSearch}
        onSelectProvider={setSelectedProvider}
        onProtocolSearchChange={setProtocolSearch}
        onModelSearchChange={setModelSearch}
        onOpenRegistryWorkbench={openRegistryWorkbench}
      />

      <ProviderEditorSheet
        open={editorOpen}
        loadingDetail={loadingDetail}
        actionStatus={actionStatus}
        actionError={actionError}
        detail={detail}
        runtimeInfo={runtimeInfo}
        health={health}
        preview={preview}
        previewing={previewing}
        saving={saving}
        selectedCapabilities={selectedCapabilities}
        formState={formState}
        refreshingProfileId={refreshingProfileId}
        onClose={() => setEditorOpen(false)}
        onRunHealthCheck={() => void runHealthCheck()}
        onRunPresentationPreview={() => void runPresentationPreview()}
        onSaveProvider={() => void saveProvider()}
        onFormStateChange={(patch) => setFormState((current) => ({ ...current, ...patch }))}
        onRefreshAuthProfile={(provider, profileId) => void refreshAuthProfile(provider, profileId)}
      />

      <ProviderRegistrySheet
        open={registryOpen}
        registryStatus={registryStatus}
        registryError={registryError}
        registryLoading={registryLoading}
        registryForm={registryForm}
        selectedProvider={selectedProvider}
        selectedRow={selectedRow}
        selectedProbeStatus={selectedCapabilities?.probe_status ?? null}
        onClose={() => setRegistryOpen(false)}
        onRegistryFormChange={(patch) => setRegistryForm((current) => ({ ...current, ...patch }))}
        onFetchModels={() => void fetchModelsIntoDraft()}
        onDeleteSelectedProvider={() => void deleteSelectedProvider()}
        onCreateProvider={() => void createProvider()}
      />

      <AuthProfileWorkbenchSheet
        open={authWorkbenchOpen}
        authLoading={authLoading}
        authStatus={authStatus}
        authError={authError}
        authSaving={authSaving}
        authEditorMode={authEditorMode}
        runtimeInfo={runtimeInfo}
        providers={data?.providers ?? []}
        authForm={authForm}
        selectedAuthProfile={selectedAuthProfile}
        selectedProfiles={selectedProfiles}
        selectedAuthProfileId={selectedAuthProfileId}
        selectedAuthProfileMode={selectedAuthProfileMode}
        connectSecret={connectSecret}
        importPath={importPath}
        deviceFlow={deviceFlow}
        importingProfileId={importingProfileId}
        refreshingProfileId={refreshingProfileId}
        connectingProfileId={connectingProfileId}
        onClose={() => setAuthWorkbenchOpen(false)}
        onStartNewDraft={startNewAuthProfileDraft}
        onImportSelectedProfile={() => void importSelectedProfile()}
        onStartBrowserOauth={() => void startBrowserOauth()}
        onStartDeviceFlow={() => void startDeviceFlow()}
        onRefreshSelectedProfile={() => void refreshAuthProfile(selectedAuthProfile?.provider ?? '', selectedAuthProfile?.id ?? '')}
        onDeleteSelectedProfile={() => void deleteSelectedProfile()}
        onSaveAuthProfile={() => void saveAuthProfile()}
        onConnectSelectedProfile={() => void connectSelectedProfile()}
        onAuthFormChange={(patch) => setAuthForm((current) => ({ ...current, ...patch }))}
        onConnectSecretChange={setConnectSecret}
        onImportPathChange={setImportPath}
        onSelectExistingProfile={(currentKey) => {
          setSelectedAuthProfileId(currentKey);
          setAuthEditorMode('edit');
        }}
      />
    </div>
  );
}

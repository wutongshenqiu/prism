import { useCallback, useEffect, useMemo, useState } from 'react';
import { useI18n } from '../i18n';
import { presentProbeStatus } from '../lib/operatorPresentation';
import { reconcileSelection } from '../lib/selection';
import { useProviderAtlasAuthWorkbench } from './provider-atlas/useProviderAtlasAuthWorkbench';
import { useProviderAtlasRegistryWorkbench } from './provider-atlas/useProviderAtlasRegistryWorkbench';
import { authProfilesApi } from '../services/authProfiles';
import { getApiErrorMessage } from '../services/errors';
import { protocolsApi } from '../services/protocols';
import { providersApi } from '../services/providers';
import type {
  AuthProfilesRuntimeResponse,
  PresentationPreviewResponse,
  ProtocolMatrixResponse,
  ProviderCapabilityEntry,
  ProviderDetail,
  ProviderHealthResult,
  ProviderTestResponse,
} from '../types/backend';
import type { ProviderAtlasResponse } from '../types/controlPlane';
import type { ProviderTestFormState } from '../components/provider-atlas/types';

interface ProviderAtlasControllerOptions {
  data: ProviderAtlasResponse | null;
  reload: () => Promise<void>;
}

export function useProviderAtlasController({
  data,
  reload,
}: ProviderAtlasControllerOptions) {
  const { t } = useI18n();
  const [selectedProvider, setSelectedProvider] = useState<string | null>(null);
  const [editorOpen, setEditorOpen] = useState(false);
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
  const [protocolSearch, setProtocolSearch] = useState('');
  const [modelSearch, setModelSearch] = useState('');
  const [testForm, setTestForm] = useState<ProviderTestFormState>({
    model: '',
    input: 'Reply with the single word ok.',
  });
  const [testingRequest, setTestingRequest] = useState(false);
  const [testResult, setTestResult] = useState<ProviderTestResponse | null>(null);
  const [testError, setTestError] = useState<string | null>(null);

  useEffect(() => {
    setSelectedProvider((current) =>
      reconcileSelection(current, data?.providers ?? [], (provider) => provider.provider),
    );
  }, [data]);

  const loadRuntimeSurfaces = useCallback(async () => {
    const [capabilities, protocols] = await Promise.all([
      providersApi.capabilities(),
      protocolsApi.matrix(),
    ]);
    setCapabilityEntries(capabilities.providers);
    setProtocolMatrix(protocols);
  }, []);

  useEffect(() => {
    let active = true;

    void (async () => {
      try {
        const [capabilities, protocols] = await Promise.all([
          providersApi.capabilities(),
          protocolsApi.matrix(),
        ]);
        if (!active) {
          return;
        }
        setCapabilityEntries(capabilities.providers);
        setProtocolMatrix(protocols);
      } catch {
        if (!active) {
          return;
        }
        setCapabilityEntries([]);
        setProtocolMatrix(null);
      }
    })();

    return () => {
      active = false;
    };
  }, [loadRuntimeSurfaces]);

  const selectedRow = useMemo(
    () => data?.providers.find((provider) => provider.provider === selectedProvider) ?? null,
    [data, selectedProvider],
  );
  const selectedCapabilities = useMemo(
    () => capabilityEntries.find((provider) => provider.name === selectedProvider) ?? null,
    [capabilityEntries, selectedProvider],
  );
  const protocolFacts = useMemo(() => {
    const endpoints = protocolMatrix?.endpoints ?? [];
    const coverage = protocolMatrix?.coverage.filter((entry) => !entry.disabled) ?? [];
    return {
      publicRoutes: endpoints.filter((entry) => entry.scope === 'public').length,
      providerRoutes: endpoints.filter((entry) => entry.scope === 'provider_scoped').length,
      nativeSurfaces: coverage.filter((entry) => entry.execution_mode === 'native').length,
      adaptedSurfaces: coverage.filter(
        (entry) => entry.execution_mode && entry.execution_mode !== 'native',
      ).length,
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
        return [entry.surface_label, entry.surface_id, entry.execution_mode ?? '', entry.upstream]
          .join(' ')
          .toLowerCase()
          .includes(needle);
      })
      .slice(0, 8);
  }, [protocolMatrix?.coverage, protocolSearch, selectedProvider]);
  const filteredModelInventory = useMemo(() => {
    const needle = modelSearch.trim().toLowerCase();
    return modelInventory.filter((item) => {
      if (!needle) {
        return true;
      }
      return [item.id, item.provider, item.upstream, item.probe]
        .join(' ')
        .toLowerCase()
        .includes(needle);
    });
  }, [modelInventory, modelSearch]);

  const registryWorkbench = useProviderAtlasRegistryWorkbench({
    selectedProvider,
    reload,
    loadRuntimeSurfaces,
    setSelectedProvider,
    setDetail,
  });

  const authWorkbench = useProviderAtlasAuthWorkbench({
    providers: data?.providers ?? [],
    selectedProvider,
    reload,
    setDetail,
    setRuntimeInfo,
  });

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
    setTestResult(null);
    setTestError(null);

    try {
      const [provider, runtime] = await Promise.all([
        providersApi.get(selectedProvider),
        authProfilesApi.runtime(),
      ]);
      setDetail(provider);
      setRuntimeInfo(runtime);
      registryWorkbench.setFormState({
        baseUrl: provider.base_url ?? '',
        region: provider.region ?? '',
        weight: String(provider.weight ?? 1),
        disabled: provider.disabled,
      });
      setTestForm({
        model: provider.models[0]?.id ?? selectedCapabilities?.models[0]?.id ?? 'gpt-5',
        input: 'Reply with the single word ok.',
      });
    } catch (editorError) {
      setActionError(getApiErrorMessage(editorError, t('providerAtlas.error.loadDetail')));
    } finally {
      setLoadingDetail(false);
    }
  };

  const runHealthCheck = async () => {
    if (!selectedProvider) {
      return;
    }
    setActionError(null);
    setActionStatus(t('providerAtlas.status.runningHealthProbe'));
    try {
      const result = await providersApi.healthCheck(selectedProvider);
      setHealth(result);
      setActionStatus(t('providerAtlas.status.healthProbeCompleted', { status: presentProbeStatus(result.status, t) }));
      await Promise.all([reload(), loadRuntimeSurfaces()]);
    } catch (probeError) {
      setActionError(getApiErrorMessage(probeError, t('providerAtlas.error.healthProbe')));
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
        user_agent: 'prism-control-plane',
        sample_body: {
          input: 'hello',
          messages: [{ role: 'user', content: 'hello' }],
        },
      });
      setPreview(result);
      setActionStatus(t('providerAtlas.status.previewGenerated', { provider: selectedProvider }));
    } catch (previewError) {
      setActionError(getApiErrorMessage(previewError, t('providerAtlas.error.preview')));
    } finally {
      setPreviewing(false);
    }
  };

  const runTestRequest = async () => {
    if (!selectedProvider) {
      return;
    }
    if (!testForm.model.trim() || !testForm.input.trim()) {
      setTestError(t('providerAtlas.error.testRequestInput'));
      return;
    }

    setTestingRequest(true);
    setActionError(null);
    setActionStatus(null);
    setTestError(null);
    setTestResult(null);
    try {
      const result = await providersApi.testRequest(selectedProvider, {
        model: testForm.model.trim(),
        input: testForm.input.trim(),
      });
      setTestResult(result);
      setActionStatus(t('providerAtlas.status.testRequestCompleted', { provider: selectedProvider }));
    } catch (requestError) {
      setTestError(getApiErrorMessage(requestError, t('providerAtlas.error.testRequest')));
    } finally {
      setTestingRequest(false);
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
        base_url: registryWorkbench.formState.baseUrl.trim() || null,
        region: registryWorkbench.formState.region.trim() || null,
        weight: Number(registryWorkbench.formState.weight) || 1,
        disabled: registryWorkbench.formState.disabled,
      });
      setActionStatus(t('providerAtlas.status.savedProvider', { provider: selectedProvider }));
      await reload();
      await loadRuntimeSurfaces();
      const refreshed = await providersApi.get(selectedProvider);
      setDetail(refreshed);
    } catch (saveError) {
      setActionError(getApiErrorMessage(saveError, t('providerAtlas.error.saveProvider')));
    } finally {
      setSaving(false);
    }
  };

  return {
    selectedProvider,
    setSelectedProvider,
    editorOpen,
    setEditorOpen,
    detail,
    health,
    preview,
    actionError,
    actionStatus,
    loadingDetail,
    saving,
    previewing,
    runtimeInfo,
    selectedCapabilities,
    protocolFacts,
    filteredProtocolCoverage,
    filteredModelInventory,
    protocolSearch,
    setProtocolSearch,
    modelSearch,
    setModelSearch,
    testForm,
    setTestForm,
    testingRequest,
    testResult,
    testError,
    selectedRow,
    providers: data?.providers ?? [],
    openEditor,
    runHealthCheck,
    runPresentationPreview,
    runTestRequest,
    saveProvider,
    ...registryWorkbench,
    ...authWorkbench,
  };
}

import { useCallback, useEffect, useMemo, useState } from 'react';
import {
  buildAuthKeyCreateRequest,
  emptyAccessForm,
  formFromAuthKey,
  type AccessPolicyFormState,
} from '../lib/authKeyPolicy';
import { clearRouteDraft, readRouteDraft, type RouteDraft } from '../lib/routeDraft';
import { reconcileSelection } from '../lib/selection';
import { authKeysApi } from '../services/authKeys';
import { configApi } from '../services/config';
import { getApiErrorMessage } from '../services/errors';
import { tenantsApi } from '../services/tenants';
import type {
  AuthKeySummary,
  ConfigApplyResponse,
  ConfigValidateResponse,
  TenantMetricsResponse,
  TenantSummary,
} from '../types/backend';
import type { AuthKeyUpdateRequest } from '../types/backend';
import type { ChangeStudioResponse } from '../types/controlPlane';

interface ChangeStudioControllerOptions {
  data: ChangeStudioResponse | null;
  reload: () => Promise<void>;
}

export function useChangeStudioController({
  data,
  reload,
}: ChangeStudioControllerOptions) {
  const [selectedFamily, setSelectedFamily] = useState<string | null>(null);
  const [editorOpen, setEditorOpen] = useState(false);
  const [editorMode, setEditorMode] = useState<'structured' | 'yaml'>('structured');
  const [yaml, setYaml] = useState('');
  const [configVersion, setConfigVersion] = useState<string | undefined>();
  const [configPath, setConfigPath] = useState('');
  const [routeDraft, setRouteDraft] = useState<RouteDraft | null>(null);
  const [loadingEditor, setLoadingEditor] = useState(false);
  const [validating, setValidating] = useState(false);
  const [applying, setApplying] = useState(false);
  const [reloading, setReloading] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [actionStatus, setActionStatus] = useState<string | null>(null);
  const [validationResult, setValidationResult] = useState<ConfigValidateResponse | null>(null);
  const [applyResult, setApplyResult] = useState<ConfigApplyResponse | null>(null);
  const [authKeys, setAuthKeys] = useState<AuthKeySummary[]>([]);
  const [tenants, setTenants] = useState<TenantSummary[]>([]);
  const [selectedTenantId, setSelectedTenantId] = useState<string | null>(null);
  const [tenantMetrics, setTenantMetrics] = useState<TenantMetricsResponse | null>(null);
  const [tenantLoading, setTenantLoading] = useState(false);
  const [tenantError, setTenantError] = useState<string | null>(null);
  const [refreshingAccess, setRefreshingAccess] = useState(false);
  const [selectedAuthKeyId, setSelectedAuthKeyId] = useState<number | null>(null);
  const [accessOpen, setAccessOpen] = useState(false);
  const [accessEditorMode, setAccessEditorMode] = useState<'create' | 'edit'>('create');
  const [accessForm, setAccessForm] = useState<AccessPolicyFormState>(emptyAccessForm);
  const [accessStatus, setAccessStatus] = useState<string | null>(null);
  const [accessError, setAccessError] = useState<string | null>(null);
  const [revealedKey, setRevealedKey] = useState<string | null>(null);
  const [savingKey, setSavingKey] = useState(false);
  const [revealingKey, setRevealingKey] = useState(false);
  const [deletingKey, setDeletingKey] = useState(false);

  useEffect(() => {
    setSelectedFamily((current) =>
      reconcileSelection(current, data?.registry ?? [], (item) => item.family),
    );
  }, [data]);

  const selectedRegistry = useMemo(
    () => data?.registry.find((item) => item.family === selectedFamily) ?? null,
    [data, selectedFamily],
  );
  const selectedAuthKey = useMemo(
    () => authKeys.find((item) => item.id === selectedAuthKeyId) ?? null,
    [authKeys, selectedAuthKeyId],
  );

  useEffect(() => {
    if (!selectedAuthKey) {
      return;
    }
    setAccessEditorMode('edit');
    setAccessForm(formFromAuthKey(selectedAuthKey));
  }, [selectedAuthKey]);

  const loadTenantMetrics = async (tenantId: string) => {
    setTenantLoading(true);
    setTenantError(null);
    try {
      const response = await tenantsApi.metrics(tenantId);
      setTenantMetrics(response);
      setSelectedTenantId(tenantId);
    } catch (loadError) {
      setTenantError(getApiErrorMessage(loadError, 'Failed to load tenant metrics'));
    } finally {
      setTenantLoading(false);
    }
  };

  const loadAccessData = useCallback(async () => {
    const [keysResponse, tenantsResponse] = await Promise.all([
      authKeysApi.list(),
      tenantsApi.list(),
    ]);
    setAuthKeys(keysResponse.auth_keys);
    setTenants(tenantsResponse.tenants);
    setSelectedAuthKeyId((current) => current ?? keysResponse.auth_keys[0]?.id ?? null);
    setSelectedTenantId((current) => current ?? tenantsResponse.tenants[0]?.id ?? null);
  }, []);

  const loadEditor = async (mode: 'structured' | 'yaml') => {
    setEditorMode(mode);
    setEditorOpen(true);
    setLoadingEditor(true);
    setActionError(null);
    setActionStatus(null);
    setValidationResult(null);
    setApplyResult(null);

    try {
      const [rawConfig] = await Promise.all([configApi.raw()]);
      setYaml(rawConfig.content);
      setConfigVersion(rawConfig.config_version);
      setConfigPath(rawConfig.path);
      setRouteDraft(readRouteDraft());
    } catch (loadError) {
      setActionError(getApiErrorMessage(loadError, 'Failed to load configuration draft'));
    } finally {
      setLoadingEditor(false);
    }
  };

  useEffect(() => {
    void loadAccessData().catch(() => {
      setAuthKeys([]);
      setTenants([]);
    });
  }, [loadAccessData]);

  useEffect(() => {
    setSelectedAuthKeyId((current) =>
      reconcileSelection(current, authKeys, (item) => item.id),
    );
  }, [authKeys]);

  useEffect(() => {
    setSelectedTenantId((current) =>
      reconcileSelection(current, tenants, (tenant) => tenant.id),
    );
  }, [tenants]);

  const refreshAccessPosture = async () => {
    setRefreshingAccess(true);
    setAccessError(null);
    setTenantError(null);
    try {
      await loadAccessData();
      if (selectedTenantId) {
        await loadTenantMetrics(selectedTenantId);
      }
    } catch (refreshError) {
      setAccessError(getApiErrorMessage(refreshError, 'Failed to refresh access posture'));
    } finally {
      setRefreshingAccess(false);
    }
  };

  const validateDraft = async () => {
    setValidating(true);
    setActionError(null);
    setActionStatus(null);
    try {
      const result = await configApi.validate(yaml);
      setValidationResult(result);
      setActionStatus(result.valid ? 'Validation passed.' : 'Validation returned issues.');
    } catch (validationError) {
      setActionError(getApiErrorMessage(validationError, 'Validation failed'));
    } finally {
      setValidating(false);
    }
  };

  const applyDraft = async () => {
    setApplying(true);
    setActionError(null);
    setActionStatus(null);
    try {
      const result = await configApi.apply(yaml, configVersion);
      setApplyResult(result);
      setConfigVersion(result.config_version);
      setActionStatus(result.message);
      await reload();
    } catch (applyError) {
      setActionError(getApiErrorMessage(applyError, 'Apply failed'));
    } finally {
      setApplying(false);
    }
  };

  const reloadRuntime = async () => {
    setReloading(true);
    setActionError(null);
    setActionStatus(null);
    try {
      const result = await configApi.reload();
      setActionStatus(result.message);
      await reload();
    } catch (reloadError) {
      setActionError(getApiErrorMessage(reloadError, 'Runtime reload failed'));
    } finally {
      setReloading(false);
    }
  };

  const openAccessWorkbench = async () => {
    setAccessOpen(true);
    setAccessError(null);
    setAccessStatus(null);
    setRevealedKey(null);
    setAccessEditorMode('create');
    setAccessForm(emptyAccessForm);
    try {
      await loadAccessData();
    } catch (loadError) {
      setAccessError(getApiErrorMessage(loadError, 'Failed to load access controls'));
    }
  };

  const startNewAccessDraft = () => {
    setAccessEditorMode('create');
    setAccessError(null);
    setAccessStatus(null);
    setRevealedKey(null);
    setAccessForm(emptyAccessForm);
  };

  const saveAuthKey = async () => {
    setSavingKey(true);
    setAccessError(null);
    setAccessStatus(null);
    setRevealedKey(null);
    try {
      const body = buildAuthKeyCreateRequest(accessForm);

      if (accessEditorMode === 'edit' && selectedAuthKeyId !== null) {
        const update: AuthKeyUpdateRequest = {
          name: body.name,
          tenant_id: accessForm.tenantId.trim() ? accessForm.tenantId.trim() : null,
          allowed_models: body.allowed_models,
          allowed_credentials: body.allowed_credentials,
          rate_limit: body.rate_limit ?? null,
          budget: body.budget ?? null,
          expires_at: body.expires_at ?? null,
        };
        await authKeysApi.update(selectedAuthKeyId, update);
        setAccessStatus(`Saved auth key ${accessForm.name || selectedAuthKeyId}.`);
      } else {
        const response = await authKeysApi.create(body);
        setAccessStatus(response.message);
        setRevealedKey(response.key);
      }
      await loadAccessData();
      const latestKeys = await authKeysApi.list();
      const matchingKey = latestKeys.auth_keys.find((item) => item.name === (body.name ?? null));
      if (matchingKey) {
        setSelectedAuthKeyId(matchingKey.id);
      }
      if (accessForm.tenantId.trim()) {
        await loadTenantMetrics(accessForm.tenantId.trim());
      }
    } catch (createError) {
      setAccessError(getApiErrorMessage(createError, 'Failed to save auth key'));
    } finally {
      setSavingKey(false);
    }
  };

  const revealAuthKey = async () => {
    if (selectedAuthKeyId === null) {
      setAccessError('Select an auth key first.');
      return;
    }

    setRevealingKey(true);
    setAccessError(null);
    setAccessStatus(null);
    try {
      const response = await authKeysApi.reveal(selectedAuthKeyId);
      setRevealedKey(response.key);
      setAccessStatus(`Revealed auth key ${selectedAuthKey?.name ?? selectedAuthKeyId}.`);
    } catch (revealError) {
      setAccessError(getApiErrorMessage(revealError, 'Failed to reveal auth key'));
    } finally {
      setRevealingKey(false);
    }
  };

  const deleteAuthKey = async () => {
    if (selectedAuthKeyId === null) {
      setAccessError('Select an auth key first.');
      return;
    }

    setDeletingKey(true);
    setAccessError(null);
    setAccessStatus(null);
    try {
      await authKeysApi.remove(selectedAuthKeyId);
      setAccessStatus(`Deleted auth key ${selectedAuthKey?.name ?? selectedAuthKeyId}.`);
      setRevealedKey(null);
      await loadAccessData();
      setSelectedAuthKeyId(null);
    } catch (deleteError) {
      setAccessError(getApiErrorMessage(deleteError, 'Failed to delete auth key'));
    } finally {
      setDeletingKey(false);
    }
  };

  const discardRouteDraft = () => {
    clearRouteDraft();
    setRouteDraft(null);
  };

  return {
    selectedFamily,
    setSelectedFamily,
    selectedRegistry,
    editorOpen,
    setEditorOpen,
    editorMode,
    yaml,
    setYaml,
    configVersion,
    configPath,
    routeDraft,
    loadingEditor,
    validating,
    applying,
    reloading,
    actionError,
    actionStatus,
    validationResult,
    applyResult,
    authKeys,
    tenants,
    selectedTenantId,
    tenantMetrics,
    tenantLoading,
    tenantError,
    refreshingAccess,
    selectedAuthKeyId,
    setSelectedAuthKeyId,
    accessOpen,
    setAccessOpen,
    accessEditorMode,
    accessForm,
    setAccessForm,
    accessStatus,
    accessError,
    revealedKey,
    savingKey,
    revealingKey,
    deletingKey,
    selectedAuthKey,
    loadEditor,
    loadTenantMetrics,
    refreshAccessPosture,
    validateDraft,
    applyDraft,
    reloadRuntime,
    openAccessWorkbench,
    startNewAccessDraft,
    saveAuthKey,
    revealAuthKey,
    deleteAuthKey,
    discardRouteDraft,
  };
}

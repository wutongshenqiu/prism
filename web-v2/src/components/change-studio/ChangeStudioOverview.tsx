import { Panel } from '../Panel';
import { StatusPill } from '../StatusPill';
import type { TenantMetricsResponse, TenantSummary, AuthKeySummary } from '../../types/backend';
import type { ChangeStudioResponse, RegistryRow } from '../../types/controlPlane';

interface ChangeStudioOverviewProps {
  loading: boolean;
  error: string | null;
  data: ChangeStudioResponse | null;
  selectedFamily: string | null;
  selectedRegistry: RegistryRow | null;
  authKeys: AuthKeySummary[];
  selectedAuthKeyId: number | null;
  tenants: TenantSummary[];
  selectedTenantId: string | null;
  tenantMetrics: TenantMetricsResponse | null;
  tenantLoading: boolean;
  tenantError: string | null;
  refreshingAccess: boolean;
  onSelectFamily: (family: string) => void;
  onOpenAccessWorkbench: () => void;
  onSelectAuthKey: (authKeyId: number) => void;
  onRefreshAccessPosture: () => void;
  onSelectTenant: (tenantId: string) => void;
}

export function ChangeStudioOverview({
  loading,
  error,
  data,
  selectedFamily,
  selectedRegistry,
  authKeys,
  selectedAuthKeyId,
  tenants,
  selectedTenantId,
  tenantMetrics,
  tenantLoading,
  tenantError,
  refreshingAccess,
  onSelectFamily,
  onOpenAccessWorkbench,
  onSelectAuthKey,
  onRefreshAccessPosture,
  onSelectTenant,
}: ChangeStudioOverviewProps) {
  return (
    <>
      {selectedRegistry ? (
        <div className="status-message status-message--warning">
          Active family: <strong>{selectedRegistry.family}</strong> · {selectedRegistry.record} · {selectedRegistry.dependents} dependents
        </div>
      ) : null}

      <div className="two-column">
        <Panel title="Config registry" subtitle="Object families should be browsable and impact-aware." className="panel--wide">
          <div className="table-grid table-grid--changes">
            <div className="table-grid__head">Family</div>
            <div className="table-grid__head">Record</div>
            <div className="table-grid__head">State</div>
            <div className="table-grid__head">Dependents</div>
            {loading && !data ? <div className="table-grid__cell">Loading registry…</div> : null}
            {error && !data ? <div className="table-grid__cell">{error}</div> : null}
            {(data?.registry ?? []).flatMap((item) => {
              const selected = item.family === selectedFamily;
              const cellClass = `table-grid__cell ${selected ? 'is-selected' : ''} is-clickable`;
              return [
                <div
                  key={`${item.family}-family`}
                  className={`${cellClass} table-grid__cell--strong`}
                  onClick={() => onSelectFamily(item.family)}
                >
                  {item.family}
                </div>,
                <div key={`${item.family}-record`} className={cellClass} onClick={() => onSelectFamily(item.family)}>
                  {item.record}
                </div>,
                <div key={`${item.family}-state`} className={cellClass} onClick={() => onSelectFamily(item.family)}>
                  <StatusPill label={item.state} tone={item.state_tone} />
                </div>,
                <div key={`${item.family}-deps`} className={cellClass} onClick={() => onSelectFamily(item.family)}>
                  {item.dependents}
                </div>,
              ];
            })}
          </div>
        </Panel>

        <Panel title="Transaction posture" subtitle="Current config transaction truth and delivery controls.">
          <ul className="fact-list">
            {(data?.publish_facts ?? []).map((fact) => (
              <li key={fact.label}><span>{fact.label}</span><strong>{fact.value}</strong></li>
            ))}
          </ul>
        </Panel>
      </div>

      <div className="two-column">
        <Panel title="Runtime access keys" subtitle="Gateway keys stay tied to tenants and can be created, revealed, and revoked without leaving the control plane.">
          <div className="inline-actions">
            <button type="button" className="button button--primary" onClick={onOpenAccessWorkbench}>
              Manage access keys
            </button>
          </div>
          <div className="table-grid table-grid--keys">
            <div className="table-grid__head">Key</div>
            <div className="table-grid__head">Name</div>
            <div className="table-grid__head">Tenant</div>
            <div className="table-grid__head">Models</div>
            {authKeys.length === 0 ? <div className="table-grid__cell">No gateway auth keys configured.</div> : null}
            {authKeys.flatMap((item) => {
              const selected = item.id === selectedAuthKeyId;
              const cellClass = `table-grid__cell ${selected ? 'is-selected' : ''} is-clickable`;
              return [
                <div key={`${item.id}-key`} className={`${cellClass} table-grid__cell--strong`} onClick={() => onSelectAuthKey(item.id)}>
                  {item.key_masked}
                </div>,
                <div key={`${item.id}-name`} className={cellClass} onClick={() => onSelectAuthKey(item.id)}>
                  {item.name ?? 'unnamed'}
                </div>,
                <div key={`${item.id}-tenant`} className={cellClass} onClick={() => onSelectAuthKey(item.id)}>
                  {item.tenant_id ?? 'global'}
                </div>,
                <div key={`${item.id}-models`} className={cellClass} onClick={() => onSelectAuthKey(item.id)}>
                  {item.allowed_models.length || 'all'}
                </div>,
              ];
            })}
          </div>
        </Panel>

        <Panel title="Tenant posture" subtitle="Tenant-scoped demand and cost should stay visible next to access control work.">
          <div className="inline-actions">
            <button type="button" className="button button--ghost" onClick={onRefreshAccessPosture} disabled={refreshingAccess}>
              {refreshingAccess ? 'Refreshing…' : 'Refresh access posture'}
            </button>
          </div>
          {tenants.length === 0 ? (
            <div className="status-message">No tenant-scoped traffic has been recorded yet.</div>
          ) : (
            <ul className="fact-list fact-list--interactive">
              {tenants.map((tenant) => (
                <li
                  key={tenant.id}
                  className={tenant.id === selectedTenantId ? 'is-selected' : ''}
                  onClick={() => onSelectTenant(tenant.id)}
                >
                  <span>{tenant.id}</span>
                  <strong>{tenant.requests} req · ${tenant.cost_usd}</strong>
                </li>
              ))}
            </ul>
          )}
          {tenantLoading ? <div className="status-message">Loading tenant metrics…</div> : null}
          {tenantError ? <div className="status-message status-message--danger">{tenantError}</div> : null}
          {tenantMetrics?.metrics ? (
            <div className="detail-grid">
              <div className="detail-grid__row"><span>Tenant</span><strong>{tenantMetrics.tenant_id}</strong></div>
              <div className="detail-grid__row"><span>Requests</span><strong>{tenantMetrics.metrics.requests}</strong></div>
              <div className="detail-grid__row"><span>Tokens</span><strong>{tenantMetrics.metrics.tokens}</strong></div>
              <div className="detail-grid__row"><span>Cost</span><strong>${tenantMetrics.metrics.cost_usd}</strong></div>
            </div>
          ) : null}
        </Panel>
      </div>
    </>
  );
}

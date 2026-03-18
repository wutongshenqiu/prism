import { Panel } from '../Panel';
import { StatusPill } from '../StatusPill';
import { useI18n } from '../../i18n';
import { presentFactValue } from '../../lib/operatorPresentation';
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
  const { t, tx, formatCurrency, formatNumber } = useI18n();

  return (
    <>
      {selectedRegistry ? (
        <div className="status-message status-message--info">
          {t('changeStudio.status.activeFamily')} <strong>{tx(selectedRegistry.family_label)}</strong> · {selectedRegistry.record} · {selectedRegistry.dependents} {t('changeStudio.status.dependents')}
        </div>
      ) : null}

      <div className="two-column">
        <Panel title={t('changeStudio.panel.registry.title')} subtitle={t('changeStudio.panel.registry.subtitle')} className="panel--wide">
          <div className="table-grid table-grid--changes">
            <div className="table-grid__head">{t('common.family')}</div>
            <div className="table-grid__head">{t('common.record')}</div>
            <div className="table-grid__head">{t('common.state')}</div>
            <div className="table-grid__head">{t('changeStudio.table.dependents')}</div>
            {loading && !data ? <div className="table-grid__cell">{t('changeStudio.loading.registry')}</div> : null}
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
                  {tx(item.family_label)}
                </div>,
                <div key={`${item.family}-record`} className={cellClass} onClick={() => onSelectFamily(item.family)}>
                  {item.record}
                </div>,
                <div key={`${item.family}-state`} className={cellClass} onClick={() => onSelectFamily(item.family)}>
                  <StatusPill label={tx(item.state)} tone={item.state_tone} />
                </div>,
                <div key={`${item.family}-deps`} className={cellClass} onClick={() => onSelectFamily(item.family)}>
                  {item.dependents}
                </div>,
              ];
            })}
          </div>
        </Panel>

        <Panel title={t('changeStudio.panel.transaction.title')} subtitle={t('changeStudio.panel.transaction.subtitle')}>
          <ul className="fact-list">
            {(data?.publish_facts ?? []).map((fact) => (
              <li key={`${fact.label.key}-${fact.value}`}><span>{tx(fact.label)}</span><strong>{presentFactValue(fact, tx)}</strong></li>
            ))}
          </ul>
        </Panel>
      </div>

      <div className="two-column">
        <Panel title={t('changeStudio.panel.accessKeys.title')} subtitle={t('changeStudio.panel.accessKeys.subtitle')}>
          <div className="inline-actions">
            <button type="button" className="button button--primary" onClick={onOpenAccessWorkbench}>
              {t('changeStudio.panel.accessKeys.manage')}
            </button>
          </div>
          <div className="table-grid table-grid--keys">
            <div className="table-grid__head">{t('changeStudio.access.key')}</div>
            <div className="table-grid__head">{t('common.name')}</div>
            <div className="table-grid__head">{t('common.tenant')}</div>
            <div className="table-grid__head">{t('common.models')}</div>
            {authKeys.length === 0 ? <div className="table-grid__cell">{t('changeStudio.access.empty')}</div> : null}
            {authKeys.flatMap((item) => {
              const selected = item.id === selectedAuthKeyId;
              const cellClass = `table-grid__cell ${selected ? 'is-selected' : ''} is-clickable`;
              return [
                <div key={`${item.id}-key`} className={`${cellClass} table-grid__cell--strong`} onClick={() => onSelectAuthKey(item.id)}>
                  {item.key_masked}
                </div>,
                <div key={`${item.id}-name`} className={cellClass} onClick={() => onSelectAuthKey(item.id)}>
                  {item.name ?? t('changeStudio.access.unnamed')}
                </div>,
                <div key={`${item.id}-tenant`} className={cellClass} onClick={() => onSelectAuthKey(item.id)}>
                  {item.tenant_id ?? t('common.global')}
                </div>,
                <div key={`${item.id}-models`} className={cellClass} onClick={() => onSelectAuthKey(item.id)}>
                  {item.allowed_models.length || t('common.all')}
                </div>,
              ];
            })}
          </div>
        </Panel>

        <Panel title={t('changeStudio.panel.tenantPosture.title')} subtitle={t('changeStudio.panel.tenantPosture.subtitle')}>
          <div className="inline-actions">
            <button type="button" className="button button--ghost" onClick={onRefreshAccessPosture} disabled={refreshingAccess}>
              {refreshingAccess ? t('changeStudio.panel.tenantPosture.refreshing') : t('changeStudio.panel.tenantPosture.refresh')}
            </button>
          </div>
          {tenants.length === 0 ? (
            <div className="status-message">{t('changeStudio.panel.tenantPosture.empty')}</div>
          ) : (
            <ul className="fact-list fact-list--interactive">
              {tenants.map((tenant) => (
                <li
                  key={tenant.id}
                  className={tenant.id === selectedTenantId ? 'is-selected' : ''}
                  onClick={() => onSelectTenant(tenant.id)}
                >
                  <span>{tenant.id}</span>
                  <strong>{formatNumber(tenant.requests)} req · {formatCurrency(tenant.cost_usd)}</strong>
                </li>
              ))}
            </ul>
          )}
          {tenantLoading ? <div className="status-message">{t('changeStudio.panel.tenantPosture.loading')}</div> : null}
          {tenantError ? <div className="status-message status-message--danger">{tenantError}</div> : null}
          {tenantMetrics?.metrics ? (
            <div className="detail-grid">
              <div className="detail-grid__row"><span>{t('common.tenant')}</span><strong>{tenantMetrics.tenant_id}</strong></div>
              <div className="detail-grid__row"><span>{t('common.requests')}</span><strong>{formatNumber(tenantMetrics.metrics.requests)}</strong></div>
              <div className="detail-grid__row"><span>{t('common.tokens')}</span><strong>{formatNumber(tenantMetrics.metrics.tokens)}</strong></div>
              <div className="detail-grid__row"><span>{t('common.cost')}</span><strong>{formatCurrency(tenantMetrics.metrics.cost_usd)}</strong></div>
            </div>
          ) : null}
        </Panel>
      </div>
    </>
  );
}

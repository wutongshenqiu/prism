import { WorkbenchSheet } from '../WorkbenchSheet';
import { useI18n } from '../../i18n';
import type { RouteExplanation } from '../../types/backend';
import type { RouteScenarioRow } from '../../types/controlPlane';

interface RouteSimulationSheetProps {
  open: boolean;
  simulationLoading: boolean;
  simulationStatus: string | null;
  simulationError: string | null;
  selectedScenario: RouteScenarioRow | null;
  explanation: RouteExplanation | null;
  onClose: () => void;
  onPromoteToChange: () => void;
  onSimulateDraft: () => void;
}

export function RouteSimulationSheet({
  open,
  simulationLoading,
  simulationStatus,
  simulationError,
  selectedScenario,
  explanation,
  onClose,
  onPromoteToChange,
  onSimulateDraft,
}: RouteSimulationSheetProps) {
  const { t } = useI18n();

  return (
    <WorkbenchSheet
      open={open}
      onClose={onClose}
      title={t('routeStudio.simulation.title')}
      subtitle={t('routeStudio.simulation.subtitle')}
      actions={(
        <>
          <button type="button" className="button button--ghost" onClick={onPromoteToChange} disabled={!selectedScenario}>
            {t('routeStudio.simulation.promote')}
          </button>
          <button type="button" className="button button--primary" onClick={onSimulateDraft} disabled={simulationLoading}>
            {simulationLoading ? t('routeStudio.simulation.simulating') : t('routeStudio.simulation.rerun')}
          </button>
        </>
      )}
    >
      {simulationStatus ? <div className="status-message status-message--success">{simulationStatus}</div> : null}
      {simulationError ? <div className="status-message status-message--danger">{simulationError}</div> : null}

      {selectedScenario ? (
        <section className="sheet-section">
          <h3>{t('routeStudio.simulation.scenarioPosture')}</h3>
          <div className="detail-grid">
            <div className="detail-grid__row"><span>{t('routeStudio.scenario.scenario')}</span><strong>{selectedScenario.scenario}</strong></div>
            <div className="detail-grid__row"><span>{t('common.model')}</span><strong>{selectedScenario.model}</strong></div>
            <div className="detail-grid__row"><span>{t('routeStudio.simulation.endpoint')}</span><strong>{selectedScenario.endpoint}</strong></div>
            <div className="detail-grid__row"><span>{t('routeStudio.simulation.sourceFormat')}</span><strong>{selectedScenario.source_format}</strong></div>
          </div>
        </section>
      ) : null}

      {explanation ? (
        <>
          <section className="sheet-section">
            <h3>{t('routeStudio.simulation.winningRoute')}</h3>
            <div className="detail-grid">
              <div className="detail-grid__row"><span>{t('common.profile')}</span><strong>{explanation.profile}</strong></div>
              <div className="detail-grid__row"><span>{t('routeStudio.simulation.matchedRule')}</span><strong>{explanation.matched_rule ?? t('common.default')}</strong></div>
              <div className="detail-grid__row"><span>{t('common.provider')}</span><strong>{explanation.selected?.provider ?? t('common.none')}</strong></div>
              <div className="detail-grid__row"><span>{t('routeStudio.simulation.credential')}</span><strong>{explanation.selected?.credential_name ?? t('common.none')}</strong></div>
            </div>
          </section>

          <section className="sheet-section">
            <h3>{t('routeStudio.simulation.alternates')}</h3>
            <div className="probe-list">
              {explanation.alternates.slice(0, 3).map((alternate) => (
                <div key={`${alternate.provider}-${alternate.credential_name}`} className="probe-check">
                  <span>{alternate.provider}</span>
                  <strong>{alternate.model}</strong>
                </div>
              ))}
              {explanation.rejections.slice(0, 3).map((rejection) => (
                <div key={`${rejection.candidate}-${JSON.stringify(rejection.reason)}`} className="probe-check">
                  <span>{rejection.candidate}</span>
                  <strong>{typeof rejection.reason === 'string' ? rejection.reason : t('trafficLab.replay.missingCapability')}</strong>
                </div>
              ))}
              {explanation.alternates.length === 0 && explanation.rejections.length === 0 ? (
                <div className="status-message">{t('trafficLab.replay.noRejections')}</div>
              ) : null}
            </div>
          </section>
        </>
      ) : null}
    </WorkbenchSheet>
  );
}

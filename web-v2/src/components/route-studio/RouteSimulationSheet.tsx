import { WorkbenchSheet } from '../WorkbenchSheet';
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
  return (
    <WorkbenchSheet
      open={open}
      onClose={onClose}
      title="Route simulation workbench"
      subtitle="Run a real explain against the selected scenario using the current local draft, then promote it into change review."
      actions={(
        <>
          <button type="button" className="button button--ghost" onClick={onPromoteToChange} disabled={!selectedScenario}>
            Promote to change
          </button>
          <button type="button" className="button button--primary" onClick={onSimulateDraft} disabled={simulationLoading}>
            {simulationLoading ? 'Simulating…' : 'Re-run simulation'}
          </button>
        </>
      )}
    >
      {simulationStatus ? <div className="status-message status-message--success">{simulationStatus}</div> : null}
      {simulationError ? <div className="status-message status-message--danger">{simulationError}</div> : null}

      {selectedScenario ? (
        <section className="sheet-section">
          <h3>Scenario posture</h3>
          <div className="detail-grid">
            <div className="detail-grid__row"><span>Scenario</span><strong>{selectedScenario.scenario}</strong></div>
            <div className="detail-grid__row"><span>Model</span><strong>{selectedScenario.model}</strong></div>
            <div className="detail-grid__row"><span>Endpoint</span><strong>{selectedScenario.endpoint}</strong></div>
            <div className="detail-grid__row"><span>Source format</span><strong>{selectedScenario.source_format}</strong></div>
          </div>
        </section>
      ) : null}

      {explanation ? (
        <>
          <section className="sheet-section">
            <h3>Winning route</h3>
            <div className="detail-grid">
              <div className="detail-grid__row"><span>Profile</span><strong>{explanation.profile}</strong></div>
              <div className="detail-grid__row"><span>Matched rule</span><strong>{explanation.matched_rule ?? 'default'}</strong></div>
              <div className="detail-grid__row"><span>Provider</span><strong>{explanation.selected?.provider ?? 'none'}</strong></div>
              <div className="detail-grid__row"><span>Credential</span><strong>{explanation.selected?.credential_name ?? 'none'}</strong></div>
            </div>
          </section>

          <section className="sheet-section">
            <h3>Alternates and rejections</h3>
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
                  <strong>{typeof rejection.reason === 'string' ? rejection.reason : 'missing capability'}</strong>
                </div>
              ))}
            </div>
          </section>
        </>
      ) : null}
    </WorkbenchSheet>
  );
}

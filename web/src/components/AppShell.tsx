import { NavLink, Outlet, useLocation, useNavigate } from 'react-router-dom';
import { Activity, ArrowRightLeft, Boxes, ChartNoAxesCombined, GitBranchPlus, Layers3, LogOut } from 'lucide-react';
import { DEFAULT_INSPECTORS, WORKSPACES } from '../constants/workspaces';
import { useI18n } from '../i18n';
import { useShellStore } from '../stores/shellStore';
import { useAuthStore } from '../stores/authStore';
import type { WorkspaceId } from '../types/shell';
import { StatusPill } from './StatusPill';

const ICONS: Record<WorkspaceId, typeof Activity> = {
  'command-center': ChartNoAxesCombined,
  'traffic-lab': Activity,
  'provider-atlas': Boxes,
  'route-studio': ArrowRightLeft,
  'change-studio': GitBranchPlus,
};

function workspaceFromPath(pathname: string): WorkspaceId {
  if (pathname.includes('traffic-lab')) return 'traffic-lab';
  if (pathname.includes('provider-atlas')) return 'provider-atlas';
  if (pathname.includes('route-studio')) return 'route-studio';
  if (pathname.includes('change-studio')) return 'change-studio';
  return 'command-center';
}

export function AppShell() {
  const location = useLocation();
  const navigate = useNavigate();
  const workspaceId = workspaceFromPath(location.pathname);
  const { t, tx } = useI18n();
  const {
    environment,
    timeRange,
    sourceMode,
    live,
    locale,
    inspectors,
    setEnvironment,
    setTimeRange,
    setSourceMode,
    toggleLive,
    toggleLocale,
  } = useShellStore();
  const username = useAuthStore((state) => state.username);
  const logout = useAuthStore((state) => state.logout);
  const inspector = inspectors[workspaceId] ?? DEFAULT_INSPECTORS[workspaceId];

  const handleInspectorAction = (action: typeof inspector.actions[number]) => {
    if (action.effect === 'navigate' && action.target_workspace) {
      navigate(`/${action.target_workspace}`);
      return;
    }
    if (action.effect === 'invoke') {
      const targetWorkspace = action.target_workspace ?? workspaceId;
      const nextSearch = new URLSearchParams(
        targetWorkspace === workspaceId ? location.search : '',
      );
      nextSearch.set('inspect_action', action.id);
      navigate({
        pathname: `/${targetWorkspace}`,
        search: `?${nextSearch.toString()}`,
      });
      return;
    }
    if (action.effect === 'reload') {
      window.location.reload();
      return;
    }
    navigate('/command-center');
  };

  return (
    <div className="shell">
      <aside className="shell-sidebar">
        <div className="brand">
          <div className="brand__mark">P</div>
          <div>
            <strong>{t('common.appName')}</strong>
            <p>{t('common.controlPlane')}</p>
          </div>
        </div>
        <nav className="workspace-nav">
          {WORKSPACES.map((workspace) => {
            const Icon = ICONS[workspace.id];
            return (
              <NavLink
                key={workspace.id}
                to={`/${workspace.id}`}
                className={({ isActive }) => `workspace-nav__item ${isActive ? 'is-active' : ''}`}
              >
                <Icon size={16} />
                <div>
                  <strong>{tx(workspace.label)}</strong>
                  <span>{tx(workspace.summary)}</span>
                </div>
              </NavLink>
            );
          })}
        </nav>
      </aside>

      <div className="shell-main">
        <header className="context-bar">
          <div className="context-bar__group">
            <label>
              <span>{t('shell.context.environment')}</span>
              <select value={environment} onChange={(event) => setEnvironment(event.target.value as typeof environment)}>
                <option value="production">{t('common.production')}</option>
                <option value="staging">{t('common.staging')}</option>
              </select>
            </label>
            <label>
              <span>{t('shell.context.range')}</span>
              <select value={timeRange} onChange={(event) => setTimeRange(event.target.value as typeof timeRange)}>
                <option value="15m">15m</option>
                <option value="1h">1h</option>
                <option value="6h">6h</option>
                <option value="24h">24h</option>
              </select>
            </label>
            <label>
              <span>{t('shell.context.source')}</span>
              <select value={sourceMode} onChange={(event) => setSourceMode(event.target.value as typeof sourceMode)}>
                <option value="runtime">{t('common.runtime')}</option>
                <option value="hybrid">{t('common.hybrid')}</option>
                <option value="external">{t('common.external')}</option>
              </select>
            </label>
          </div>

          <div className="context-bar__actions">
            <button className={`button ${live ? 'button--primary' : 'button--ghost'}`} onClick={toggleLive}>
              {live ? t('common.live') : t('common.pausedTitle')}
            </button>
            <button className="button button--ghost" onClick={toggleLocale}>
              {locale === 'zh-CN' ? t('common.locale.switchToEn') : t('common.locale.switchToZh')}
            </button>
            <button className="button button--ghost context-user" onClick={() => void logout()}>
              <span>{username ?? t('shell.context.userFallback')}</span>
              <LogOut size={14} />
            </button>
          </div>
        </header>

        <div className="shell-content">
          <main className="workspace-canvas">
            <Outlet />
          </main>

          <aside className="inspector">
            <div className="inspector__header">
              <p className="workspace-eyebrow">{tx(inspector.eyebrow)}</p>
              <h2>{tx(inspector.title)}</h2>
              <p>{tx(inspector.summary)}</p>
            </div>

            {inspector.sections.map((section) => (
              <section key={section.title.key} className="inspector-section">
                <h3>{tx(section.title)}</h3>
                <ul>
                  {section.rows.map((row) => (
                    <li key={`${row.label.key}-${row.value}`}>
                      <span>{tx(row.label)}</span>
                      <strong>{row.value_text ? tx(row.value_text) : row.value}</strong>
                    </li>
                  ))}
                </ul>
              </section>
            ))}

            <section className="inspector-section">
              <h3>{t('shell.inspector.nextActions')}</h3>
              <div className="action-stack">
                {inspector.actions.map((action) => (
                  <button
                    key={action.id}
                    type="button"
                    className="button button--secondary button--block"
                    onClick={() => handleInspectorAction(action)}
                  >
                    {tx(action.label)}
                  </button>
                ))}
              </div>
            </section>

            <section className="inspector-section">
              <h3>{t('shell.inspector.sourcePosture')}</h3>
              <div className="source-posture">
                <StatusPill
                  label={sourceMode === 'runtime' ? t('common.runtime') : sourceMode === 'hybrid' ? t('common.hybrid') : t('common.external')}
                  tone={sourceMode === 'hybrid' ? 'warning' : sourceMode === 'external' ? 'info' : 'success'}
                />
                <span>{environment === 'production' ? t('common.production') : t('common.staging')} / {timeRange}</span>
              </div>
              <div className="inspector-note">
                <Layers3 size={16} />
                <span>{t('shell.inspector.note')}</span>
              </div>
            </section>
          </aside>
        </div>
      </div>
    </div>
  );
}

import { NavLink, Outlet, useNavigate } from 'react-router-dom';
import { useAuthStore } from '../stores/authStore';
import { useWebSocket } from '../hooks/useWebSocket';
import {
  LayoutDashboard,
  FileText,
  Server,
  Network,
  Layers,
  GitBranch,
  FileCode,
  Users,
  KeyRound,
  Activity,
  ScrollText,
  LogOut,
  Menu,
  X,
  PlayCircle,
} from 'lucide-react';
import { useState } from 'react';

interface NavSection {
  label: string;
  items: { to: string; icon: React.ComponentType<{ size?: number }>; label: string }[];
}

const navSections: NavSection[] = [
  {
    label: 'Observe',
    items: [
      { to: '/', icon: LayoutDashboard, label: 'Overview' },
      { to: '/requests', icon: FileText, label: 'Requests' },
    ],
  },
  {
    label: 'Infrastructure',
    items: [
      { to: '/protocols', icon: Network, label: 'Protocols' },
      { to: '/providers', icon: Server, label: 'Providers' },
      { to: '/models', icon: Layers, label: 'Models & Capabilities' },
    ],
  },
  {
    label: 'Traffic',
    items: [
      { to: '/routing', icon: GitBranch, label: 'Routing' },
      { to: '/replay', icon: PlayCircle, label: 'Replay' },
    ],
  },
  {
    label: 'Access Control',
    items: [
      { to: '/tenants', icon: Users, label: 'Tenants & Keys' },
      { to: '/auth-profiles', icon: KeyRound, label: 'Auth Profiles' },
    ],
  },
  {
    label: 'Operations',
    items: [
      { to: '/config', icon: FileCode, label: 'Config & Changes' },
      { to: '/system', icon: Activity, label: 'System' },
      { to: '/logs', icon: ScrollText, label: 'App Logs' },
    ],
  },
];

export default function Layout() {
  const logout = useAuthStore((s) => s.logout);
  const navigate = useNavigate();
  const [sidebarOpen, setSidebarOpen] = useState(false);

  // Connect WebSocket for real-time updates
  const { connectionState } = useWebSocket();

  const connectionLabel = connectionState === 'connected'
    ? 'Realtime connected'
    : connectionState === 'connecting'
      ? 'Realtime reconnecting'
      : 'Realtime disconnected';
  const connectionClass = connectionState === 'connected'
    ? 'type-badge type-badge--green'
    : connectionState === 'connecting'
      ? 'type-badge type-badge--blue'
      : 'type-badge type-badge--red';

  const handleLogout = () => {
    logout();
    navigate('/login');
  };

  return (
    <div className="layout">
      {/* Mobile overlay */}
      {sidebarOpen && (
        <div
          className="sidebar-overlay"
          onClick={() => setSidebarOpen(false)}
        />
      )}

      {/* Sidebar */}
      <aside className={`sidebar ${sidebarOpen ? 'sidebar--open' : ''}`}>
        <div className="sidebar-header">
          <div className="sidebar-logo">
            <Activity size={24} />
            <span>Prism</span>
          </div>
          <button
            className="sidebar-close"
            onClick={() => setSidebarOpen(false)}
          >
            <X size={20} />
          </button>
        </div>

        <nav className="sidebar-nav">
          {navSections.map((section) => (
            <div key={section.label} className="sidebar-section">
              <div className="sidebar-section-label">{section.label}</div>
              {section.items.map(({ to, icon: Icon, label }) => (
                <NavLink
                  key={to}
                  to={to}
                  end={to === '/'}
                  className={({ isActive }) =>
                    `sidebar-nav-item ${isActive ? 'sidebar-nav-item--active' : ''}`
                  }
                  onClick={() => setSidebarOpen(false)}
                >
                  <Icon size={18} />
                  <span>{label}</span>
                </NavLink>
              ))}
            </div>
          ))}
        </nav>

        <div className="sidebar-footer">
          <button className="sidebar-nav-item" onClick={handleLogout}>
            <LogOut size={18} />
            <span>Logout</span>
          </button>
        </div>
      </aside>

      {/* Main content */}
      <main className="main-content">
        <header className="main-header">
          <button
            className="mobile-menu-btn"
            onClick={() => setSidebarOpen(true)}
          >
            <Menu size={20} />
          </button>
          <h1 className="main-header-title">Prism Gateway</h1>
          <div className="main-header-status">
            <span className={connectionClass}>
              <span className={`live-dot ${connectionState === 'connected' ? 'live-dot--active' : ''}`} />
              {connectionLabel}
            </span>
          </div>
        </header>
        <div className="main-body">
          <Outlet />
        </div>
      </main>
    </div>
  );
}

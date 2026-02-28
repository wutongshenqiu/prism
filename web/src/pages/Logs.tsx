import { useEffect, useState, useCallback } from 'react';
import { systemApi } from '../services/api';
import type { SystemLog } from '../types';
import {
  ScrollText,
  ChevronLeft,
  ChevronRight,
  Search,
  RefreshCw,
} from 'lucide-react';

type LogLevel = '' | 'TRACE' | 'DEBUG' | 'INFO' | 'WARN' | 'ERROR';

export default function Logs() {
  const [logs, setLogs] = useState<SystemLog[]>([]);
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(0);
  const [isLoading, setIsLoading] = useState(true);
  const [levelFilter, setLevelFilter] = useState<LogLevel>('');
  const [searchQuery, setSearchQuery] = useState('');

  const fetchLogs = useCallback(async () => {
    setIsLoading(true);
    try {
      const response = await systemApi.logs(page, levelFilter || undefined);
      setLogs(response.data.data);
      setTotalPages(response.data.total_pages);
    } catch (err) {
      console.error('Failed to fetch system logs:', err);
    } finally {
      setIsLoading(false);
    }
  }, [page, levelFilter]);

  useEffect(() => {
    fetchLogs();
  }, [fetchLogs]);

  const handleLevelChange = (level: LogLevel) => {
    setLevelFilter(level);
    setPage(1);
  };

  const getLevelClass = (level: string): string => {
    switch (level) {
      case 'ERROR': return 'log-level--error';
      case 'WARN': return 'log-level--warn';
      case 'INFO': return 'log-level--info';
      case 'DEBUG': return 'log-level--debug';
      case 'TRACE': return 'log-level--trace';
      default: return '';
    }
  };

  const filteredLogs = searchQuery
    ? logs.filter(
        (log) =>
          log.message.toLowerCase().includes(searchQuery.toLowerCase()) ||
          log.target.toLowerCase().includes(searchQuery.toLowerCase())
      )
    : logs;

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h2>Application Logs</h2>
          <p className="page-subtitle">System and application log viewer</p>
        </div>
        <button className="btn btn-secondary" onClick={fetchLogs}>
          <RefreshCw size={16} />
          Refresh
        </button>
      </div>

      {/* Filters */}
      <div className="card" style={{ marginBottom: '1.5rem' }}>
        <div className="card-body">
          <div className="filter-bar">
            <div className="filter-group">
              <select
                value={levelFilter}
                onChange={(e) => handleLevelChange(e.target.value as LogLevel)}
                className="filter-input"
              >
                <option value="">All Levels</option>
                <option value="ERROR">ERROR</option>
                <option value="WARN">WARN</option>
                <option value="INFO">INFO</option>
                <option value="DEBUG">DEBUG</option>
                <option value="TRACE">TRACE</option>
              </select>
              <div className="search-input-wrapper">
                <Search size={14} className="search-icon" />
                <input
                  type="text"
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  placeholder="Search logs..."
                  className="filter-input search-input"
                />
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Log Viewer */}
      <div className="card">
        <div className="log-viewer">
          {isLoading ? (
            <div className="log-viewer-loading">Loading logs...</div>
          ) : filteredLogs.length === 0 ? (
            <div className="empty-state">
              <ScrollText size={48} />
              <p>No logs found</p>
            </div>
          ) : (
            <div className="log-entries">
              {filteredLogs.map((log, index) => (
                <div key={index} className="log-entry">
                  <span className="log-timestamp">
                    {new Date(log.timestamp).toLocaleString()}
                  </span>
                  <span className={`log-level ${getLevelClass(log.level)}`}>
                    {log.level}
                  </span>
                  <span className="log-target">{log.target}</span>
                  <span className="log-message">{log.message}</span>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Pagination */}
        {totalPages > 1 && (
          <div className="pagination">
            <button
              className="btn btn-secondary btn-sm"
              disabled={page <= 1}
              onClick={() => setPage(page - 1)}
            >
              <ChevronLeft size={14} />
              Prev
            </button>
            <span className="pagination-info">
              Page {page} of {totalPages}
            </span>
            <button
              className="btn btn-secondary btn-sm"
              disabled={page >= totalPages}
              onClick={() => setPage(page + 1)}
            >
              Next
              <ChevronRight size={14} />
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

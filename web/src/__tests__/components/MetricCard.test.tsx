import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import MetricCard from '../../components/MetricCard';

describe('MetricCard', () => {
  it('renders title and value', () => {
    render(
      <MetricCard title="Total Requests" value={1234} icon={<span>icon</span>} />
    );
    expect(screen.getByText('Total Requests')).toBeInTheDocument();
    expect(screen.getByText('1234')).toBeInTheDocument();
  });

  it('renders subtitle', () => {
    render(
      <MetricCard title="Latency" value="250ms" subtitle="avg" icon={<span>icon</span>} />
    );
    expect(screen.getByText('avg')).toBeInTheDocument();
  });

  it('renders trend with up indicator', () => {
    render(
      <MetricCard title="RPM" value={10} trend="up" trendValue="15%" icon={<span>icon</span>} />
    );
    expect(screen.getByText('+15%')).toBeInTheDocument();
  });

  it('renders trend with down indicator', () => {
    render(
      <MetricCard title="Errors" value={5} trend="down" trendValue="3%" icon={<span>icon</span>} />
    );
    expect(screen.getByText('-3%')).toBeInTheDocument();
  });

  it('renders neutral trend without prefix', () => {
    render(
      <MetricCard title="T" value={0} trend="neutral" trendValue="0%" icon={<span>icon</span>} />
    );
    expect(screen.getByText('0%')).toBeInTheDocument();
  });

  it('does not render trend without trendValue', () => {
    const { container } = render(
      <MetricCard title="T" value={0} trend="up" icon={<span>icon</span>} />
    );
    expect(container.querySelector('.metric-card-trend')).not.toBeInTheDocument();
  });

  it('applies color class to icon', () => {
    const { container } = render(
      <MetricCard title="T" value={0} color="red" icon={<span>icon</span>} />
    );
    expect(container.querySelector('.metric-card-icon--red')).toBeInTheDocument();
  });

  it('defaults to blue color', () => {
    const { container } = render(
      <MetricCard title="T" value={0} icon={<span>icon</span>} />
    );
    expect(container.querySelector('.metric-card-icon--blue')).toBeInTheDocument();
  });
});

import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import StatusBadge from '../../components/StatusBadge';

describe('StatusBadge', () => {
  it('renders healthy status with default label', () => {
    render(<StatusBadge status="healthy" />);
    expect(screen.getByText('Healthy')).toBeInTheDocument();
  });

  it('renders custom label', () => {
    render(<StatusBadge status="healthy" label="Running" />);
    expect(screen.getByText('Running')).toBeInTheDocument();
  });

  it('renders unhealthy with red class', () => {
    const { container } = render(<StatusBadge status="unhealthy" />);
    expect(container.querySelector('.status-badge--red')).toBeInTheDocument();
    expect(screen.getByText('Unhealthy')).toBeInTheDocument();
  });

  it('renders degraded with yellow class', () => {
    const { container } = render(<StatusBadge status="degraded" />);
    expect(container.querySelector('.status-badge--yellow')).toBeInTheDocument();
  });

  it('renders inactive with gray class', () => {
    const { container } = render(<StatusBadge status="inactive" />);
    expect(container.querySelector('.status-badge--gray')).toBeInTheDocument();
    expect(screen.getByText('Inactive')).toBeInTheDocument();
  });

  it('renders up with green class', () => {
    const { container } = render(<StatusBadge status="up" />);
    expect(container.querySelector('.status-badge--green')).toBeInTheDocument();
    expect(screen.getByText('Up')).toBeInTheDocument();
  });

  it('renders down with red class', () => {
    const { container } = render(<StatusBadge status="down" />);
    expect(container.querySelector('.status-badge--red')).toBeInTheDocument();
    expect(screen.getByText('Down')).toBeInTheDocument();
  });

  it('renders dot indicator', () => {
    const { container } = render(<StatusBadge status="active" />);
    expect(container.querySelector('.status-badge-dot')).toBeInTheDocument();
  });
});

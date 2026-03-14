import { describe, it, expect, beforeEach, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

const mockProvidersApi = vi.hoisted(() => ({
  list: vi.fn(),
  create: vi.fn(),
  update: vi.fn(),
  delete: vi.fn(),
  fetchModels: vi.fn(),
  healthCheck: vi.fn(),
}));

vi.mock('../../services/api', () => ({
  providersApi: mockProvidersApi,
}));

const { default: Providers } = await import('../../pages/Providers');

describe('Providers page', () => {
  beforeEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it('shows success feedback after creating a Claude provider', async () => {
    const user = userEvent.setup();
    mockProvidersApi.list
      .mockResolvedValueOnce({ data: [] })
      .mockResolvedValueOnce({
        data: [
          {
            name: 'claude-prod',
            format: 'claude',
            api_key_masked: 'sk-a****test',
            base_url: 'https://api.anthropic.com',
            models_count: 0,
            disabled: false,
          },
        ],
      });
    mockProvidersApi.create.mockResolvedValueOnce({
      data: { message: 'Provider created successfully' },
    });

    render(<Providers />);

    await screen.findByText('No providers configured');
    await user.click(screen.getByRole('button', { name: /add first provider/i }));
    await user.type(
      screen.getByPlaceholderText('e.g., deepseek, openai-prod'),
      'claude-prod'
    );
    const [formatSelect] = screen.getAllByRole('combobox');
    await user.selectOptions(formatSelect, 'claude');
    await user.type(screen.getByPlaceholderText('sk-...'), 'sk-ant-test-123');
    await user.click(screen.getByRole('button', { name: 'Create' }));

    await waitFor(() => {
      expect(mockProvidersApi.create).toHaveBeenCalledWith(
        expect.objectContaining({
          name: 'claude-prod',
          format: 'claude',
        })
      );
    });
    expect(
      await screen.findByText('Provider "claude-prod" created successfully.')
    ).toBeInTheDocument();
    expect(await screen.findByText('claude-prod')).toBeInTheDocument();
  });

  it('shows a warning when creation succeeds but the list refresh fails', async () => {
    const user = userEvent.setup();
    mockProvidersApi.list
      .mockResolvedValueOnce({ data: [] })
      .mockRejectedValueOnce({
        response: { data: { message: 'refresh failed' } },
        message: 'Request failed',
      });
    mockProvidersApi.create.mockResolvedValueOnce({
      data: { message: 'Provider created successfully' },
    });

    render(<Providers />);

    await screen.findByText('No providers configured');
    await user.click(screen.getByRole('button', { name: /add first provider/i }));
    await user.type(
      screen.getByPlaceholderText('e.g., deepseek, openai-prod'),
      'claude-prod'
    );
    const [formatSelect] = screen.getAllByRole('combobox');
    await user.selectOptions(formatSelect, 'claude');
    await user.type(screen.getByPlaceholderText('sk-...'), 'sk-ant-test-123');
    await user.click(screen.getByRole('button', { name: 'Create' }));

    expect(
      await screen.findByText(
        'Provider "claude-prod" created successfully, but refreshing the list failed: refresh failed'
      )
    ).toBeInTheDocument();
  });
});

import { reconcileSelection } from './selection';
import {
  resolveDeviceFlowProfileLabel,
  type DeviceFlowState,
} from './authProfileDraft';

describe('selection utilities', () => {
  it('keeps the current selection when it is still present', () => {
    const items = [{ key: 'alpha' }, { key: 'beta' }];
    expect(reconcileSelection('beta', items, (item) => item.key)).toBe('beta');
  });

  it('falls back to the first available item when the current selection disappears', () => {
    const items = [{ key: 'gamma' }, { key: 'delta' }];
    expect(reconcileSelection('beta', items, (item) => item.key)).toBe('gamma');
  });

  it('returns null when there are no items to select', () => {
    expect(reconcileSelection('beta', [], (item: { key: string }) => item.key)).toBeNull();
  });

  it('resolves device-flow labels from the completed profile when available', () => {
    const deviceFlow: Pick<DeviceFlowState, 'target_qualified_name'> = {
      target_qualified_name: 'alpha/profile-a',
    };
    expect(
      resolveDeviceFlowProfileLabel(
        deviceFlow,
        { qualified_name: 'alpha/profile-b' },
      ),
    ).toBe('alpha/profile-b');
  });

  it('falls back to the started device-flow target label', () => {
    const deviceFlow: Pick<DeviceFlowState, 'target_qualified_name'> = {
      target_qualified_name: 'alpha/profile-a',
    };
    expect(
      resolveDeviceFlowProfileLabel(
        deviceFlow,
        null,
      ),
    ).toBe('alpha/profile-a');
  });
});

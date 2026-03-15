import { useEffect, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { LoaderCircle, ShieldCheck, ShieldX } from 'lucide-react';
import { authProfilesApi } from '../services/api';
import type { AuthProfile } from '../types';
import { extractApiErrorMessage } from '../utils/apiError';

const completionRequests = new Map<string, Promise<AuthProfile>>();

export default function AuthProfileCallback() {
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const [state, setState] = useState<'loading' | 'success' | 'error'>('loading');
  const [message, setMessage] = useState('Completing OAuth login...');

  useEffect(() => {
    const oauthState = searchParams.get('state');
    const code = searchParams.get('code');
    const error = searchParams.get('error');

    if (error) {
      setState('error');
      setMessage(`OAuth provider returned an error: ${error}`);
      return;
    }
    if (!oauthState || !code) {
      setState('error');
      setMessage('Missing OAuth state or code.');
      return;
    }
    const operationKey = `${oauthState}:${code}`;
    let completion = completionRequests.get(operationKey);
    if (!completion) {
      completion = authProfilesApi.completeCodexOauth(oauthState, code);
      completionRequests.set(operationKey, completion);
    }

    let cancelled = false;
    void completion
      .then((profile) => {
        if (cancelled) return;
        setState('success');
        setMessage(`Connected ${profile.qualified_name}. Redirecting back to auth profiles...`);
        const next = new URLSearchParams({
          provider: profile.provider,
          profile: profile.id,
          oauth: 'success',
        });
        window.setTimeout(() => {
          navigate(`/auth-profiles?${next.toString()}`, { replace: true });
        }, 1200);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setState('error');
        setMessage(extractApiErrorMessage(err, 'Failed to complete OAuth login.'));
      })
      .finally(() => {
        completionRequests.delete(operationKey);
      });

    return () => {
      cancelled = true;
    };
  }, [navigate, searchParams]);

  return (
    <div className="page">
      <div className="card" style={{ maxWidth: 640, margin: '2rem auto' }}>
        <div className="card-body" style={{ textAlign: 'center', padding: '3rem 2rem' }}>
          {state === 'loading' && <LoaderCircle size={40} className="spinning" style={{ marginBottom: 16 }} />}
          {state === 'success' && <ShieldCheck size={40} style={{ marginBottom: 16, color: 'var(--color-success)' }} />}
          {state === 'error' && <ShieldX size={40} style={{ marginBottom: 16, color: 'var(--color-danger)' }} />}
          <h2 style={{ marginBottom: 12 }}>OAuth Callback</h2>
          <p className="page-subtitle" style={{ marginBottom: 20 }}>{message}</p>
          {state === 'error' && (
            <button className="btn btn-primary" onClick={() => navigate('/auth-profiles')}>
              Back to Auth Profiles
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

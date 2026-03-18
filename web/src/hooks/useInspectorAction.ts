import { useEffect, useRef } from 'react';
import { useSearchParams } from 'react-router-dom';

type InspectorActionHandler = () => void | Promise<void>;

export function useInspectorAction(
  handlers: Partial<Record<string, InspectorActionHandler>>,
) {
  const [searchParams, setSearchParams] = useSearchParams();
  const inspectAction = searchParams.get('inspect_action');
  const handlersRef = useRef(handlers);
  const handledActionRef = useRef<string | null>(null);

  useEffect(() => {
    handlersRef.current = handlers;
  }, [handlers]);

  useEffect(() => {
    if (!inspectAction) {
      handledActionRef.current = null;
      return;
    }

    if (handledActionRef.current === inspectAction) {
      return;
    }

    handledActionRef.current = inspectAction;
    const handler = handlersRef.current[inspectAction];
    const nextSearch = new URLSearchParams(searchParams);
    nextSearch.delete('inspect_action');

    let cancelled = false;

    void Promise.resolve(handler?.())
      .catch(() => undefined)
      .finally(() => {
        if (cancelled) {
          return;
        }
        setSearchParams(nextSearch, { replace: true });
      });

    return () => {
      cancelled = true;
    };
  }, [inspectAction, searchParams, setSearchParams]);
}

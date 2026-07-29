import { useEffect, useMemo, useState } from 'react';
import { useParams } from 'react-router-dom';
import { mentionsService } from '../services';
import { useContactsStore } from '../stores';
import { normalizeQualifiedRelayName } from '../utils/namedWall';
import { ContactWallPage } from './ContactWall';

type Resolution =
  | { status: 'loading' }
  | { status: 'ready'; peerId: string }
  | { status: 'unavailable'; message: string };

export function NamedContactWallPage() {
  const { qualifiedName: routeName } = useParams<{ qualifiedName: string }>();
  const loadContacts = useContactsStore((state) => state.loadContacts);
  const qualifiedName = useMemo(() => {
    if (!routeName) return null;
    try {
      return normalizeQualifiedRelayName(decodeURIComponent(routeName));
    } catch {
      return null;
    }
  }, [routeName]);
  const [resolution, setResolution] = useState<Resolution>({ status: 'loading' });

  useEffect(() => {
    let cancelled = false;
    if (!qualifiedName) {
      setResolution({ status: 'unavailable', message: 'This profile link is malformed.' });
      return;
    }
    const accountName = qualifiedName;

    async function resolve() {
      try {
        await loadContacts();
        if (cancelled) return;
        const knownContact = useContactsStore
          .getState()
          .contacts.find((contact) => contact.verifiedQualifiedName === accountName);
        if (knownContact) {
          setResolution({ status: 'ready', peerId: knownContact.peerId });
          return;
        }

        const result = await mentionsService.resolve(accountName);
        if (cancelled) return;
        if (result.status === 'known' && result.peerId) {
          setResolution({ status: 'ready', peerId: result.peerId });
        } else {
          setResolution({
            status: 'unavailable',
            message:
              'Harbor could not resolve this account from the current relay. Check your connection and try again.',
          });
        }
      } catch {
        if (!cancelled)
          setResolution({
            status: 'unavailable',
            message:
              'Harbor could not resolve this account from the current relay. Check your connection and try again.',
          });
      }
    }

    setResolution({ status: 'loading' });
    void resolve();
    return () => {
      cancelled = true;
    };
  }, [loadContacts, qualifiedName]);

  if (resolution.status === 'ready' && qualifiedName) {
    return (
      <ContactWallPage
        peerIdOverride={resolution.peerId}
        verifiedQualifiedNameOverride={qualifiedName}
      />
    );
  }

  return (
    <div
      className="h-full flex items-center justify-center p-6"
      style={{ background: 'hsl(var(--harbor-bg-primary))' }}
    >
      <div className="max-w-md text-center space-y-3">
        <h1 className="text-xl font-semibold" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
          {qualifiedName || 'Named account'}
        </h1>
        <p role={resolution.status === 'unavailable' ? 'alert' : 'status'}>
          {resolution.status === 'loading'
            ? 'Finding this account…'
            : resolution.status === 'unavailable'
              ? resolution.message
              : 'Opening this account…'}
        </p>
      </div>
    </div>
  );
}

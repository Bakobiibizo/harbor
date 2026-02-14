/**
 * Shared formatting and display utility functions.
 * Extracted from Chat.tsx, Network.tsx, and Settings.tsx to avoid duplication.
 */

/** Extract up to 2 initials from a display name. */
export function getInitials(name: string): string {
  return name
    .split(' ')
    .map((word) => word[0])
    .join('')
    .toUpperCase()
    .slice(0, 2);
}

/** Generate a consistent gradient color from a peer ID hash. */
export function getContactColor(peerId: string): string {
  const colors = [
    'linear-gradient(135deg, hsl(220 91% 54%), hsl(262 83% 58%))',
    'linear-gradient(135deg, hsl(262 83% 58%), hsl(330 81% 60%))',
    'linear-gradient(135deg, hsl(152 69% 40%), hsl(180 70% 45%))',
    'linear-gradient(135deg, hsl(36 90% 55%), hsl(15 80% 55%))',
    'linear-gradient(135deg, hsl(200 80% 50%), hsl(220 91% 54%))',
    'linear-gradient(135deg, hsl(340 75% 55%), hsl(10 80% 60%))',
    'linear-gradient(135deg, hsl(280 70% 50%), hsl(320 75% 55%))',
    'linear-gradient(135deg, hsl(170 65% 45%), hsl(200 70% 50%))',
  ];
  let hash = 0;
  for (let characterIndex = 0; characterIndex < peerId.length; characterIndex++) {
    hash = peerId.charCodeAt(characterIndex) + ((hash << 5) - hash);
  }
  return colors[Math.abs(hash) % colors.length];
}

/** Format a Date as a relative time string (e.g. "now", "5m", "2h", "3d"). */
export function formatRelativeTime(date: Date): string {
  const now = new Date();
  const diff = now.getTime() - date.getTime();
  const mins = Math.floor(diff / 60000);
  const hours = Math.floor(diff / 3600000);
  const days = Math.floor(diff / 86400000);

  if (mins < 1) return 'now';
  if (mins < 60) return `${mins}m`;
  if (hours < 24) return `${hours}h`;
  return `${days}d`;
}

/** Format a Date as a human-friendly relative string (e.g. "Just now", "5m ago", "3d ago"). */
export function formatDate(date: Date): string {
  const now = new Date();
  const diff = now.getTime() - date.getTime();
  const mins = Math.floor(diff / 60000);
  const hours = Math.floor(diff / 3600000);
  const days = Math.floor(diff / 86400000);

  if (mins < 1) return 'Just now';
  if (mins < 60) return `${mins}m ago`;
  if (hours < 24) return `${hours}h ago`;
  if (days < 7) return `${days}d ago`;
  return date.toLocaleDateString('en-US', {
    month: 'short',
    day: 'numeric',
    year: date.getFullYear() !== now.getFullYear() ? 'numeric' : undefined,
  });
}

/** Format a unix timestamp (seconds) as a relative time string. */
export function formatTimeAgo(unixSeconds: number): string {
  const now = Date.now();
  const date = new Date(unixSeconds * 1000);
  const diff = now - date.getTime();

  const minutes = Math.floor(diff / 60000);
  if (minutes < 1) return 'Just now';
  if (minutes < 60) return `${minutes}m ago`;

  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;

  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d ago`;

  return date.toLocaleDateString();
}

/** Truncate a peer ID for display: first 12 chars + last 6. */
export function shortPeerId(peerId: string): string {
  if (peerId.length <= 20) return peerId;
  return `${peerId.slice(0, 12)}...${peerId.slice(-6)}`;
}

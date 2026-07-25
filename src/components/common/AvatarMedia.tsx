import { useMediaUrl } from '../../hooks/useMediaUrl';

export function AvatarMedia({
  hash,
  className = 'w-full h-full object-cover',
  alt = '',
}: {
  hash: string;
  className?: string;
  alt?: string;
}) {
  const url = useMediaUrl(hash);
  return url ? <img src={url} alt={alt} className={className} /> : null;
}

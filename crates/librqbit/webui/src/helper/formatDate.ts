export function formatRelativeTime(dateString?: string | null): string {
  if (!dateString) return "-";
  const date = new Date(dateString);
  // check invalid date
  if (isNaN(date.getTime())) return "-";

  const now = new Date();
  const diff = now.getTime() - date.getTime();
  const seconds = Math.floor(diff / 1000);

  if (seconds < 5) return "Just now";
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days}d ago`;
  
  // Format as date if older
  return date.toLocaleDateString();
}

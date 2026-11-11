/** The Sompio mark. The same drawing is the app icon (brand/sompio-logo.svg), so it looks the same everywhere. */
export default function Logo({ size = 28, className = "" }: { size?: number; className?: string }) {
  return (
    <svg className={`logo ${className}`} width={size} height={size} viewBox="0 0 120 120" aria-hidden focusable="false">
      <rect width="120" height="120" rx="26" fill="#1f2d3a" />
      <path d="M22 58 A30 30 0 0 1 82 58 Z" fill="#ffffff" />
      <path d="M38 64 A30 30 0 0 0 98 64 Z" fill="#4caf50" />
    </svg>
  );
}

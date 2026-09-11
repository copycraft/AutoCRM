/** A feature that is not built yet. Says so plainly — never poses as an empty data set (plan §5 M5). */
export function UnavailableState({ title, body }: { title: string; body: string }) {
  return (
    <div className="border-l-2 border-steel-200 bg-surface px-5 py-4" role="status">
      <p className="text-body font-medium">{title}</p>
      <p className="mt-1 text-metadata text-steel-500">{body}</p>
    </div>
  );
}

import { JobApplyForm } from '@/components/recruitment/JobApplyForm';

// The public application page a job listing's link points at. No AppShell, no login: the
// visitor is an applicant, not staff.
export default async function JobApplyPage({ params }: { params: Promise<{ slug: string }> }) {
  const { slug } = await params;
  return <JobApplyForm slug={slug} />;
}

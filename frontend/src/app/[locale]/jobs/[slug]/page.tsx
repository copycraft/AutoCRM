import { JobApplyForm } from '@/components/recruitment/JobApplyForm';

// The public application page a job listing's link points at. No AppShell, no login: the
// visitor is an applicant, not staff.
export default function JobApplyPage({ params: { slug } }: { params: { slug: string } }) {
  return <JobApplyForm slug={slug} />;
}

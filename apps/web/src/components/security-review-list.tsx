import { securityReviewAnswers } from "@/lib/trust-center";

export function SecurityReviewList({ detail = false }: { detail?: boolean }) {
  return (
    <dl className="mx-auto mt-8 max-w-2xl divide-y divide-[#eadfce] text-left">
      {securityReviewAnswers.map((item) => (
        <div key={item.question} className="py-5 first:pt-0 last:pb-0">
          <dt className="text-base font-medium text-[#181613]">
            {item.question}
          </dt>
          <dd className="mt-2 text-sm leading-6 text-[#4f4940]">
            {detail ? item.detail : item.summary}
          </dd>
        </div>
      ))}
    </dl>
  );
}

import { Trans, useLingui } from "@lingui/react/macro";
import { useForm } from "@tanstack/react-form";
import { useMutation } from "@tanstack/react-query";

import { Button } from "@anlg/ui/components/ui/button";
import { Input } from "@anlg/ui/components/ui/input";
import { Textarea } from "@anlg/ui/components/ui/textarea";

import { formatProfilePhone } from "./phone";

import { useAuth } from "~/auth";
import { AvatarUploadButton, ContactImage } from "~/contacts/contact-avatar";
import { ContactOrganizationSelector } from "~/contacts/details";
import {
  type HumanRecord,
  savePersonalContact,
  usePersonalContact,
  useOrganizations,
} from "~/contacts/queries";
import { ContactFacehash } from "~/contacts/shared";
import { useOwnerUserId } from "~/shared/owner-user";

export function AccountProfile() {
  const auth = useAuth();
  const localOwnerUserId = useOwnerUserId();
  const humanId = auth.session?.user.id ?? localOwnerUserId;
  if (!humanId)
    return (
      <p role="status">
        <Trans>Loading...</Trans>
      </p>
    );
  return <ProfileLoader key={humanId} humanId={humanId} />;
}

function ProfileLoader({ humanId }: { humanId: string }) {
  const { data, isLoading, error } = usePersonalContact(humanId);
  if (error)
    return (
      <p role="alert">
        <Trans>
          Couldn't load your profile. Reopen this page to try again.
        </Trans>
      </p>
    );
  if (isLoading || data === undefined)
    return (
      <p role="status">
        <Trans>Loading...</Trans>
      </p>
    );
  return <ProfileForm humanId={humanId} human={data} />;
}

function ProfileForm({
  humanId,
  human,
}: {
  humanId: string;
  human: HumanRecord | null;
}) {
  const { t } = useLingui();
  const auth = useAuth();
  const organizations = useOrganizations();
  const save = useMutation({
    mutationFn: (values: Parameters<typeof savePersonalContact>[1]) =>
      savePersonalContact(humanId, {
        ...values,
        phone: formatProfilePhone(values.phone, navigator.language),
      }),
  });
  const metadataName = auth.session?.user.user_metadata?.full_name;
  const form = useForm({
    defaultValues: {
      name:
        human?.name ?? (typeof metadataName === "string" ? metadataName : ""),
      email: human?.email ?? auth.session?.user.email ?? "",
      phone: formatProfilePhone(human?.phone ?? "", navigator.language),
      jobTitle: human?.jobTitle ?? "",
      linkedinUsername: human?.linkedinUsername ?? "",
      memo: human?.memo ?? "",
      organizationId: human?.organizationId ?? "",
      avatarDataUrl: human?.avatarDataUrl ?? (null as string | null),
    },
    listeners: {
      onChange: ({ formApi }) => save.mutate(formApi.state.values),
    },
    onSubmit: ({ value }) => save.mutate(value),
  });
  const fields = [
    { name: "name", label: t`Name`, type: "text" },
    { name: "jobTitle", label: t`Job Title`, type: "text" },
    { name: "email", label: t`Email`, type: "email" },
    { name: "phone", label: t`Phone`, type: "tel" },
    { name: "linkedinUsername", label: t`LinkedIn`, type: "text" },
  ] as const;

  return (
    <form
      className="flex flex-col gap-4"
      onSubmit={(event) => {
        event.preventDefault();
        void form.handleSubmit();
      }}
    >
      <h3 className="text-sm font-medium">
        <Trans>Your info</Trans>
      </h3>
      <p className="text-muted-foreground text-sm">
        <Trans>
          Update your personal contact card. Your sign-in email is managed
          separately.
        </Trans>
      </p>
      <fieldset className="flex min-w-0 flex-col gap-4">
        <form.Field name="avatarDataUrl">
          {(field) => (
            <div className="flex items-center gap-4">
              <AvatarUploadButton
                label={t`Change photo`}
                onUpload={field.handleChange}
              >
                {field.state.value ? (
                  <ContactImage src={field.state.value} size={64} />
                ) : (
                  <ContactFacehash
                    name={human?.name || human?.email || humanId}
                    size={64}
                  />
                )}
              </AvatarUploadButton>
              {field.state.value && (
                <Button
                  variant="outline"
                  type="button"
                  onClick={() => field.handleChange(null)}
                >
                  <Trans>Remove photo</Trans>
                </Button>
              )}
            </div>
          )}
        </form.Field>
        {fields.map(({ name, label, type }) => (
          <form.Field key={name} name={name}>
            {(field) => (
              <label className="flex flex-col gap-2 text-sm">
                {label}
                <Input
                  type={type}
                  value={field.state.value}
                  onChange={(event) => field.handleChange(event.target.value)}
                  onBlur={() => {
                    if (name === "phone") {
                      field.handleChange(
                        formatProfilePhone(
                          field.state.value,
                          navigator.language,
                        ),
                      );
                    }
                    field.handleBlur();
                  }}
                  placeholder={name === "phone" ? "+1 202 555 0123" : undefined}
                />
              </label>
            )}
          </form.Field>
        ))}
        <form.Field name="organizationId">
          {(field) => (
            <div className="flex flex-col gap-2 text-sm">
              <span>
                <Trans>Company</Trans>
              </span>
              <div>
                <ContactOrganizationSelector
                  organization={
                    organizations.find(
                      (organization) => organization.id === field.state.value,
                    ) ?? null
                  }
                  organizations={organizations}
                  onChange={(id) => field.handleChange(id ?? "")}
                />
              </div>
            </div>
          )}
        </form.Field>
        <form.Field name="memo">
          {(field) => (
            <label className="flex flex-col gap-2 text-sm">
              <Trans>Notes</Trans>
              <Textarea
                value={field.state.value}
                onChange={(event) => field.handleChange(event.target.value)}
                onBlur={field.handleBlur}
                rows={3}
              />
            </label>
          )}
        </form.Field>
      </fieldset>
      {save.isError && (
        <div className="flex items-center gap-3">
          <p role="alert" className="text-destructive text-sm">
            <Trans>Couldn't save your profile. Try again.</Trans>
          </p>
          <Button type="submit" variant="outline">
            <Trans>Retry</Trans>
          </Button>
        </div>
      )}
      {(save.isPending || save.isSuccess) && (
        <span role="status" className="text-muted-foreground text-sm">
          {save.isPending ? t`Saving...` : t`Saved`}
        </span>
      )}
    </form>
  );
}

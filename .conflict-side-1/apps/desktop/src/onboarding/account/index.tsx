import { AfterLogin } from "./after-login";
import { BeforeLogin } from "./before-login";

import { useAuth } from "~/auth";

export function LoginSection({
  onContinue,
  onSkip,
}: {
  onContinue: () => void;
  onSkip?: () => void;
}) {
  const auth = useAuth();

  if (auth?.session) {
    return <AfterLogin onContinue={onContinue} />;
  }

  return (
    <BeforeLogin
      onContinue={() => {
        onSkip?.();
        onContinue();
      }}
    />
  );
}

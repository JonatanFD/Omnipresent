import { ArrowLeft } from "lucide-react";
import { useCarousel } from "../ui/carousel";
import { Button } from "../ui/button";
import { Linux } from "../ui/svgs/linux";
import MacOS from "../ui/svgs/macos";
import Windows from "../ui/svgs/windows";
import { useState } from "react";
import { Checkbox } from "../ui/checkbox";
import { saveInLocalStorage } from "@/lib/utils";
import { LocalStorageKeys } from "@/lib/types";
import { useNavigate } from "react-router-dom";
import { Sparkles } from "lucide-react";
import { Link } from "react-router-dom";

export function WelcomeSlide3() {
  const { scrollPrev } = useCarousel();
  const navigate = useNavigate();
  const [isTermsAccepted, setIsTermsAccepted] = useState(false);

  const handleStart = () => {
    saveInLocalStorage(LocalStorageKeys.ACEPTED_TERMS, "true");
    navigate("/");
  };

  const platforms = [
    { icon: Windows, label: "Windows" },
    { icon: MacOS, label: "macOS" },
    { icon: Linux, label: "Linux" },
  ];

  return (
    <article className="min-h-screen flex flex-col items-center justify-between px-8 py-16 bg-background relative overflow-hidden">
      {/* Radial glow */}
      <div
        className="pointer-events-none absolute inset-0 opacity-15"
        style={{
          background:
            "radial-gradient(ellipse 70% 40% at 50% 50%, hsl(var(--primary) / 0.4), transparent)",
        }}
      />

      <StepDots current={2} total={3} />

      {/* Platform icons */}
      <div className="flex-1 flex flex-col items-center justify-center gap-10 animate-slide-up">
        <div className="flex items-end gap-10">
          {platforms.map(({ icon: Icon, label }) => (
            <div key={label} className="flex flex-col items-center gap-3">
              <div className="p-4 rounded-2xl bg-muted/50 border border-border/50">
                <Icon className="size-12" />
              </div>
              <span className="text-xs text-muted-foreground font-medium">
                {label}
              </span>
            </div>
          ))}
        </div>

        <div className="text-center space-y-2 max-w-sm">
          <h2 className="text-2xl font-semibold tracking-tight">
            Works everywhere
          </h2>
          <p className="text-muted-foreground text-sm leading-relaxed">
            Omnipresent runs natively on Windows, macOS and Linux. One app, all
            platforms.
          </p>
        </div>
      </div>

      {/* Terms + CTA */}
      <div className="w-full max-w-sm space-y-5">
        {/* Terms checkbox */}
        <button
          type="button"
          onClick={() => setIsTermsAccepted((v) => !v)}
          className="w-full flex items-center gap-3 px-4 py-3 rounded-xl border transition-colors hover:bg-muted/50 cursor-pointer"
          style={{
            borderColor: isTermsAccepted
              ? "hsl(var(--primary) / 0.5)"
              : "hsl(var(--border))",
            background: isTermsAccepted
              ? "hsl(var(--primary) / 0.05)"
              : undefined,
          }}
        >
          <Checkbox
            checked={isTermsAccepted}
            onCheckedChange={(v) => setIsTermsAccepted(Boolean(v))}
            className="pointer-events-none"
          />
          <span className="text-sm text-muted-foreground">
            I accept the{" "}
            <Link
              to="/legal/terms"
              className="text-foreground underline underline-offset-2 hover:opacity-80"
            >
              terms and conditions
            </Link>
          </span>
        </button>

        {/* Nav row */}
        <div className="flex items-center justify-between">
          <Button
            variant="ghost"
            size="icon"
            onClick={scrollPrev}
            className="rounded-full h-10 w-10"
          >
            <ArrowLeft className="w-4 h-4" />
          </Button>

          <Button
            size="default"
            onClick={handleStart}
            disabled={!isTermsAccepted}
            className="gap-2 px-6 rounded-full"
          >
            <Sparkles className="w-4 h-4" />
            Start using Omnipresent
          </Button>
        </div>
      </div>
    </article>
  );
}

function StepDots({ current, total }: { current: number; total: number }) {
  return (
    <div className="flex items-center gap-1.5">
      {Array.from({ length: total }).map((_, i) => (
        <span
          key={i}
          className={`block rounded-full transition-all duration-300 ${
            i === current
              ? "w-5 h-1.5 bg-foreground"
              : "w-1.5 h-1.5 bg-muted-foreground/30"
          }`}
        />
      ))}
    </div>
  );
}

import Omnipresent from "../ui/svgs/omnipresent";
import { useCarousel } from "../ui/carousel";
import { Button } from "../ui/button";
import { ArrowRight } from "lucide-react";

export function WelcomeSlide1() {
  const { scrollNext } = useCarousel();

  return (
    <article className="min-h-screen flex flex-col items-center justify-between px-8 py-16 bg-background relative overflow-hidden">
      {/* Subtle radial glow behind logo */}
      <div
        className="pointer-events-none absolute inset-0 opacity-20"
        style={{
          background:
            "radial-gradient(ellipse 60% 50% at 50% 40%, hsl(var(--primary) / 0.35), transparent)",
        }}
      />

      {/* Step indicator */}
      <StepDots current={0} total={3} />

      {/* Center content */}
      <div className="flex-1 flex flex-col items-center justify-center gap-8 animate-slide-up">
        <div className="relative">
          <div className="absolute inset-0 blur-3xl opacity-30 scale-150 bg-primary rounded-full" />
          <Omnipresent className="size-28 relative" />
        </div>

        <div className="text-center space-y-3 max-w-sm">
          <h1 className="text-4xl font-semibold tracking-tight">Omnipresent</h1>
          <p className="text-muted-foreground text-base leading-relaxed">
            Control your PC from anywhere, using any device.
          </p>
        </div>
      </div>

      {/* CTA */}
      <Button
        onClick={scrollNext}
        size="lg"
        className="gap-2 px-8 rounded-full"
      >
        Get started
        <ArrowRight className="w-4 h-4" />
      </Button>
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

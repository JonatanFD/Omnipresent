import { ArrowLeft, ArrowRight } from "lucide-react";
import { useCarousel } from "../ui/carousel";
import { Button } from "../ui/button";
import { Android } from "../ui/android";
import { Safari } from "../ui/safari";

export function WelcomeSlide2() {
  const { scrollNext, scrollPrev } = useCarousel();
  return (
    <article className="min-h-screen flex flex-col">
      <div className="bg-muted h-120 flex justify-center items-center gap-20">
        <Android className="w-40 h-auto -rotate-90" />
        <div className="w-120 h-auto">
          <Safari mode="simple" />
        </div>
      </div>
      <div className="flex-1 space-x-4 flex p-4">
        <div className="flex items-center">
          <Button onClick={scrollPrev} size="icon-xl">
            <ArrowLeft />
          </Button>
        </div>

        <section className="flex-1 px-4 h-full items-center">
          <p>Control your PC right from your smartphone.</p>
        </section>

        <div className="flex items-center">
          <Button onClick={scrollNext} size="icon-xl">
            <ArrowRight />
          </Button>
        </div>
      </div>
    </article>
  );
}

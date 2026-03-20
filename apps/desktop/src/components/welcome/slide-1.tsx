import Omnipresent from "../ui/svgs/omnipresent";
import { useCarousel } from "../ui/carousel";
import { Button } from "../ui/button";

export function WelcomeSlide1() {
  const { scrollNext } = useCarousel();
  return (
    <article className="min-h-screen flex flex-col">
      <div className="bg-muted h-120 flex justify-center items-center flex-col gap-4">
        <Omnipresent className="size-40" />
        <h1 className="text-4xl font-bold">Welcome to Omnipresent</h1>
      </div>
      <div className="flex-1 flex justify-center items-center">
        <Button onClick={scrollNext} size="xl">
          Next
        </Button>
      </div>
    </article>
  );
}

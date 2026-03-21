import { ArrowLeft } from "lucide-react";
import { useCarousel } from "../ui/carousel";
import { Button } from "../ui/button";
import { Linux } from "../ui/svgs/linux";
import MacOS from "../ui/svgs/macos";
import Windows from "../ui/svgs/windows";
import { useState } from "react";
import { Label } from "../ui/label";
import { Checkbox } from "../ui/checkbox";
import { saveInLocalStorage } from "@/lib/utils";
import { LocalStorageKeys } from "@/lib/types";
import { useNavigate } from "react-router-dom";

export function WelcomeSlide3() {
  const { scrollPrev } = useCarousel();
  const navigate = useNavigate();

  const [isTermsAccepted, setIsTermsAccepted] = useState(false);

  const handleStart = () => {
    saveInLocalStorage(LocalStorageKeys.ACEPTED_TERMS, "true");
    navigate("/");
  };

  return (
    <article className="min-h-screen flex flex-col">
      <div className="bg-muted h-120 flex justify-center items-center flex-col gap-10">
        <ul className="flex items-center gap-20">
          <li>
            <Windows className="size-20" />
          </li>
          <li>
            <MacOS className="size-20" />
          </li>
          <li>
            <Linux className="size-20" />
          </li>
        </ul>
        <h1 className="text-4xl font-bold">Multiplatform</h1>
      </div>
      <div className="flex-1 space-x-4 flex p-4">
        <div className="flex items-center">
          <Button onClick={scrollPrev} size="icon-xl">
            <ArrowLeft />
          </Button>
        </div>

        <section className="flex-1 px-4 h-full items-center flex flex-col justify-center gap-4">
          <p>Omnipresent works fine on all platforms.</p>

          <Label>
            <Checkbox
              checked={isTermsAccepted}
              onCheckedChange={setIsTermsAccepted}
            />
            Accept terms and conditions
          </Label>
        </section>

        <div className="flex items-center">
          <Button onClick={handleStart} size="xl" disabled={!isTermsAccepted}>
            Start
          </Button>
        </div>
      </div>
    </article>
  );
}

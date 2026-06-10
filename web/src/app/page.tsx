import { Footer } from "@/components/sections/Footer";
import { Hero } from "@/components/sections/Hero";
import { HowItWorks } from "@/components/sections/HowItWorks";

export default function Home() {
  return (
    <main className="sm:px-6 sm:py-6">
      {/* Thin "page frame" echoing the Dell catalog sheet. */}
      <div className="sm:border sm:border-line">
        <div className="relative mx-auto max-w-[980px] px-5">
          <Hero />

          <HowItWorks />
          <Footer />
        </div>
      </div>
    </main>
  );
}

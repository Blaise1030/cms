import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@/components/ui/accordion";

export type FaqItem = { question: string; answer: string };

export function FaqAccordion({ items }: { items: FaqItem[] }) {
  return (
    <Accordion type="single" collapsible className="border-t border-line">
      {items.map((faq, index) => (
        <AccordionItem key={faq.question} value={`item-${index}`} className="border-line">
          <AccordionTrigger className="py-5 text-lg text-ink hover:no-underline">
            {faq.question}
          </AccordionTrigger>
          <AccordionContent className="max-w-prose pb-6 text-base text-muted">
            {faq.answer}
          </AccordionContent>
        </AccordionItem>
      ))}
    </Accordion>
  );
}

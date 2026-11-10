// import React, { useCallback, useState } from "react";
// import {
//   StepperRoot,
//   StepperItem,
//   StepperSeparator,
//   StepperTrigger,
//   StepperTitle,
//   StepperDescription,
// } from "@/components/ui/stepper";
// import { Button } from "@base-ui/react/button";
// import {
//   CheckCircleIcon,
//   RadioButtonIcon,
//   DotIcon,
// } from "@phosphor-icons/react";

// type Step = { step: number; title: string; description: string };

// const steps: Step[] = [
//   { step: 1, title: "选择文件", description: "选择要导入的 Excel 文件" },
//   { step: 2, title: "上传并校验", description: "上传文件并完成后端校验" },
//   { step: 3, title: "确认执行", description: "确认并开始导入" },
// ];

// export function ImportWizard() {
//   const [activeStep, setActiveStep] = React.useState<number>(1);
//   const [file, setFile] = React.useState<File | null>(null);
//   const [uploadOk, setUploadOk] = React.useState(false);
//   const [error, setError] = React.useState<string | null>(null);

//   const beforeNext = useCallback(
//     async (currentStep: number) => {
//       setError(null);
//       if (currentStep === 1) {
//         if (!file) {
//           setError("请先选择一个文件。");
//           return false;
//         }
//         return true;
//       }
//       if (currentStep === 2) {
//         if (!file) {
//           setError("文件丢失，请返回上一步重新选择。");
//           return false;
//         }
//         await new Promise((r) => setTimeout(r, 600));
//         setUploadOk(true);
//         return true;
//       }
//       return true;
//     },
//     [file],
//   );

//   return (
//     <div className="grid grid-cols-1 gap-6 md:grid-cols-[360px_1fr]">
//       <StepperRoot
//         steps={steps.length}
//         activeStep={activeStep}
//         onActiveStepChange={setActiveStep}
//         beforeNext={beforeNext}
//       >
//         {steps.map((s) => (
//           <StepperItem key={s.step} step={s.step}>
//             {({ state, step, isClickable }) => (
//               <>
//                 {step !== steps.length && (
//                   <StepperSeparator
//                     className="data-[state=completed]:bg-green-400 data-[state=active]:bg-white/80 W-0.5 h-[76px] absolute left-[15px] top-[32px] "
//                     data-state={state}
//                   />
//                 )}
//                 <StepperTrigger step={step} asChild className="rounded-full ">
//                   <Button
//                     type="button"
//                     size="icon"
//                     variant={
//                       state === "completed" || state === "active"
//                         ? "default"
//                         : "outline"
//                     }
//                     className="h-9 w-9 rounded-full"
//                     disabled={!isClickable}
//                   >
//                     {state === "completed" ? (
//                       <CheckCircleIcon size={32} />
//                     ) : state === "active" ? (
//                       <RadioButtonIcon size={32} />
//                     ) : (
//                       <DotIcon size={32} />
//                     )}
//                   </Button>
//                 </StepperTrigger>

//                 <div className="flex flex-col gap-1">
//                   <StepperTitle>{s.title}</StepperTitle>
//                   <StepperDescription className="text-muted-foreground">
//                     {s.description}
//                   </StepperDescription>
//                 </div>
//               </>
//             )}
//           </StepperItem>
//         ))}
//       </StepperRoot>

//       <div className="rounded-lg border bg-card p-4">
//         {activeStep === 1 && (
//           <div>
//             <label className="block mb-2 text-sm">选择文件</label>
//             <input
//               type="file"
//               accept=".xlsx,.xls"
//               onChange={(e) => setFile(e.target.files?.[0] ?? null)}
//             />
//             {file && (
//               <div className="mt-2 text-sm text-muted-foreground">
//                 已选择：{file.name}
//               </div>
//             )}
//           </div>
//         )}
//         {activeStep === 2 && (
//           <div className="mt-2 text-sm text-muted-foreground">
//             上传并校验中...
//           </div>
//         )}
//         {activeStep === 3 && (
//           <div className="mt-2 text-sm text-muted-foreground">准备执行导入</div>
//         )}
//         {error && (
//           <div className="mt-2 rounded border border-destructive/40 bg-destructive/10 p-2 text-sm text-destructive">
//             {error}
//           </div>
//         )}
//       </div>

//       <div className="flex justify-end gap-2 mt-4">
//         <Button
//           variant="outline"
//           onClick={() => setActiveStep((v) => Math.max(1, v - 1))}
//           disabled={activeStep === 1}
//         >
//           上一步
//         </Button>
//         <Button
//           onClick={() => {
//             beforeNext(activeStep).then(
//               (ok) => ok && setActiveStep((s) => Math.min(steps.length, s + 1)),
//             );
//           }}
//           disabled={activeStep === steps.length}
//         >
//           下一步
//         </Button>
//       </div>
//     </div>
//   );
// }

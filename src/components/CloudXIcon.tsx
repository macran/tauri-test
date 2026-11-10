import * as React from "react";

interface CloudXIconProps
  extends React.SVGProps<SVGSVGElement> {
  cloudColor?: string;
  xColor?: string;
}

export default function CloudXIcon({
  cloudColor = "currentColor",
  xColor = "#dc2626",
  ...props
}: CloudXIconProps) {
  return (
    <svg
      viewBox="0 0 256 256"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      {...props}
    >
      <path
        d="M80,128a80,80,0,1,1,80,80H72A56,56,0,1,1,85.92,97.74"
        stroke={cloudColor}
        strokeWidth="16"
        strokeLinecap="round"
        strokeLinejoin="round"
      />

      <line
        x1="184"
        y1="112"
        x2="136"
        y2="160"
        stroke={xColor}
        strokeWidth="16"
        strokeLinecap="round"
        strokeLinejoin="round"
      />

      <line
        x1="136"
        y1="112"
        x2="184"
        y2="160"
        stroke={xColor}
        strokeWidth="16"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
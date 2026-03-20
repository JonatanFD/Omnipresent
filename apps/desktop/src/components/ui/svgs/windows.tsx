interface WindowsProps extends React.SVGProps<SVGSVGElement> {}

export default function Windows(props: WindowsProps) {
  return (
    <svg
      width="29"
      height="29"
      viewBox="0 0 29 29"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      {...props}
    >
      <path
        d="M13.7666 29H1.70605C0.763922 29 0 28.2361 0 27.2939V14.9971H13.7666V29ZM29 27.2939C29 28.2361 28.2361 29 27.2939 29H15.2334V14.9971H29V27.2939ZM13.7666 13.5762H0V1.70605C0 0.763922 0.763922 0 1.70605 0H13.7666V13.5762ZM27.2939 0C28.2361 0 29 0.763922 29 1.70605V13.5762H15.2334V0H27.2939Z"
        fill="url(#paint0_linear_1602_1845)"
      />
      <defs>
        <linearGradient
          id="paint0_linear_1602_1845"
          x1="-0.104819"
          y1="-0.0355391"
          x2="29.4853"
          y2="28.4967"
          gradientUnits="userSpaceOnUse"
        >
          <stop stop-color="#8AECF6" />
          <stop offset="0.376995" stop-color="#24B8E7" />
          <stop offset="1" stop-color="#3774C4" />
        </linearGradient>
      </defs>
    </svg>
  );
}

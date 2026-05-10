export interface EmailProviderPreset {
  id: string;
  name: string;
  imap: { host: string; port: number };
  pop3: { host: string; port: number };
  smtp?: { host: string; port: number };
  defaultAuthMethod: "password" | "oauth2";
  defaultSsl: boolean;
  defaultProtocol: "imap" | "pop3";
  /** e.g. "163.com" — used for username placeholder */
  emailDomain: string;
  /** Coremail servers require ID command before SELECT */
  requiresIdCommand: boolean;
  /** Whether this provider uses authorization codes instead of login passwords */
  usesAuthorizationCode: boolean;
  /** Hint shown below password field */
  passwordHint?: string;
  /** Default folder */
  defaultFolder: string;
  /** No POP3 support */
  pop3Unsupported?: boolean;
}

export const EMAIL_PRESETS: EmailProviderPreset[] = [
  {
    id: "163",
    name: "163 邮箱",
    imap: { host: "imap.163.com", port: 993 },
    pop3: { host: "pop.163.com", port: 995 },
    smtp: { host: "smtp.163.com", port: 465 },
    defaultAuthMethod: "password",
    defaultSsl: true,
    defaultProtocol: "imap",
    emailDomain: "163.com",
    requiresIdCommand: true,
    usesAuthorizationCode: true,
    passwordHint: "请使用授权码，非登录密码（在邮箱设置→POP3/IMAP 中开启并获取）",
    defaultFolder: "INBOX",
  },
  {
    id: "qq",
    name: "QQ 邮箱",
    imap: { host: "imap.qq.com", port: 993 },
    pop3: { host: "pop.qq.com", port: 995 },
    smtp: { host: "smtp.qq.com", port: 465 },
    defaultAuthMethod: "password",
    defaultSsl: true,
    defaultProtocol: "imap",
    emailDomain: "qq.com",
    requiresIdCommand: true,
    usesAuthorizationCode: true,
    passwordHint: "请使用授权码，非QQ密码（在邮箱设置→账户→POP3/IMAP 中开启）",
    defaultFolder: "INBOX",
  },
  {
    id: "yeah",
    name: "Yeah 邮箱",
    imap: { host: "imap.yeah.net", port: 993 },
    pop3: { host: "pop.yeah.net", port: 995 },
    smtp: { host: "smtp.yeah.net", port: 465 },
    defaultAuthMethod: "password",
    defaultSsl: true,
    defaultProtocol: "imap",
    emailDomain: "yeah.net",
    requiresIdCommand: true,
    usesAuthorizationCode: true,
    passwordHint: "请使用授权码，非登录密码",
    defaultFolder: "INBOX",
  },
  {
    id: "126",
    name: "126 邮箱",
    imap: { host: "imap.126.com", port: 993 },
    pop3: { host: "pop.126.com", port: 995 },
    smtp: { host: "smtp.126.com", port: 465 },
    defaultAuthMethod: "password",
    defaultSsl: true,
    defaultProtocol: "imap",
    emailDomain: "126.com",
    requiresIdCommand: true,
    usesAuthorizationCode: true,
    passwordHint: "请使用授权码，非登录密码",
    defaultFolder: "INBOX",
  },
  {
    id: "sina",
    name: "新浪邮箱",
    imap: { host: "imap.sina.com", port: 993 },
    pop3: { host: "pop3.sina.com", port: 995 },
    smtp: { host: "smtp.sina.com", port: 465 },
    defaultAuthMethod: "password",
    defaultSsl: true,
    defaultProtocol: "imap",
    emailDomain: "sina.com",
    requiresIdCommand: true,
    usesAuthorizationCode: true,
    passwordHint: "请使用授权码，非登录密码（在设置→账户 中开启IMAP/POP3）",
    defaultFolder: "INBOX",
  },
  {
    id: "foxmail",
    name: "Foxmail / QQ 企业邮箱",
    imap: { host: "imap.exmail.qq.com", port: 993 },
    pop3: { host: "pop.exmail.qq.com", port: 995 },
    smtp: { host: "smtp.exmail.qq.com", port: 465 },
    defaultAuthMethod: "password",
    defaultSsl: true,
    defaultProtocol: "imap",
    emailDomain: "exmail.qq.com",
    requiresIdCommand: true,
    usesAuthorizationCode: false,
    defaultFolder: "INBOX",
  },
  {
    id: "gmail",
    name: "Gmail",
    imap: { host: "imap.gmail.com", port: 993 },
    pop3: { host: "pop.gmail.com", port: 995 },
    smtp: { host: "smtp.gmail.com", port: 587 },
    defaultAuthMethod: "oauth2",
    defaultSsl: true,
    defaultProtocol: "imap",
    emailDomain: "gmail.com",
    requiresIdCommand: false,
    usesAuthorizationCode: false,
    passwordHint: "需要 OAuth2 认证或应用专用密码（普通密码已禁用）",
    defaultFolder: "INBOX",
  },
  {
    id: "outlook",
    name: "Outlook / Hotmail",
    imap: { host: "outlook.office365.com", port: 993 },
    pop3: { host: "outlook.office365.com", port: 995 },
    smtp: { host: "smtp.office365.com", port: 587 },
    defaultAuthMethod: "oauth2",
    defaultSsl: true,
    defaultProtocol: "imap",
    emailDomain: "outlook.com",
    requiresIdCommand: false,
    usesAuthorizationCode: false,
    passwordHint: "需要 OAuth2 认证或应用密码（基本认证已弃用）",
    defaultFolder: "INBOX",
  },
  {
    id: "yahoo",
    name: "Yahoo 邮箱",
    imap: { host: "imap.mail.yahoo.com", port: 993 },
    pop3: { host: "pop.mail.yahoo.com", port: 995 },
    smtp: { host: "smtp.mail.yahoo.com", port: 587 },
    defaultAuthMethod: "password",
    defaultSsl: true,
    defaultProtocol: "imap",
    emailDomain: "yahoo.com",
    requiresIdCommand: false,
    usesAuthorizationCode: false,
    passwordHint: "需要使用应用专用密码（在账户安全设置中生成）",
    defaultFolder: "INBOX",
  },
  {
    id: "icloud",
    name: "iCloud 邮箱",
    imap: { host: "imap.mail.me.com", port: 993 },
    pop3: { host: "pop.mail.me.com", port: 995 },
    smtp: { host: "smtp.mail.me.com", port: 587 },
    defaultAuthMethod: "password",
    defaultSsl: true,
    defaultProtocol: "imap",
    emailDomain: "icloud.com",
    requiresIdCommand: false,
    usesAuthorizationCode: false,
    passwordHint: "需要使用 App-Specific Password（在 Apple ID 设置中生成）",
    defaultFolder: "INBOX",
    pop3Unsupported: true,
  },
];

export function getPresetById(id: string): EmailProviderPreset | undefined {
  return EMAIL_PRESETS.find((p) => p.id === id);
}

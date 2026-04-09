use clap::Parser;

#[derive(Parser, Debug, Clone)]
pub struct ListArgs {
    #[arg(default_value = "/", help = "远程目录路径")]
    pub path: String,

    #[arg(short, long, default_value = "1", help = "页码")]
    pub page: i32,

    #[arg(short = 's', long, default_value = "100", help = "每页数量")]
    pub page_size: i32,

    #[arg(short, long, help = "将JSON输出到指定文件")]
    pub output: Option<String>,
}

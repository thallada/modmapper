use anyhow::Result;
use reqwest::Client;
use std::fs::File;
use std::time::Duration;
use tokio::time::sleep;

pub async fn download_tiles(dir: &str) -> Result<()> {
    let client = Client::builder().build()?;
    for z in 10..18 {
        for x in 0..2_u32.pow(z - 9) {
            for y in 0..2_u32.pow(z - 9) {
                sleep(Duration::from_millis(100)).await;
                let url = format!(
                    "https://maps.uesp.net/srmap/color/zoom{z}/skyrim-{x}-{y}-{z}.jpg",
                    z = z,
                    x = x,
                    y = y
                );
                let resp = client.get(&url).send().await?;
                if resp.status().is_success() {
                    println!("{}", url);
                    std::fs::create_dir_all(format!("{}/{z}/{x}", dir, z = z, x = x))?;
                    let mut out =
                        File::create(format!("{}/{z}/{x}/{y}.jpg", dir, z = z, x = x, y = y))?;
                    let bytes = resp.bytes().await?;
                    std::io::copy(&mut bytes.as_ref(), &mut out)?;
                }
            }
        }
    }
    Ok(())
}

#include <iostream>
#include <map>
#include <cmath>
#include <TFile.h>
#include <TTree.h>
#include <TStopwatch.h>
#include <TApplication.h>
#include <TBranch.h>
#include <TCanvas.h>
#include <TClassTable.h>
#include <TTreeReader.h>
#include <TTreePerfStats.h>
#include <TROOT.h>
#include <TRootCanvas.h>
#include <TStyle.h>
#include <TSystem.h>
#include <TH1D.h>

/// adapted from https://github.com/jblomer/iotools/blob/master/lhcb.cxx

static void TreeQuery(TTree* tree, bool measure, bool show) {
  auto ts_init = std::chrono::steady_clock::now();

  TTreePerfStats *ps = measure ? (new TTreePerfStats("ioperf", tree)) : nullptr;

  /*
WITH m AS (
SELECT SQRT("H1_PX" * "H1_PX" + "H1_PY" * "H1_PY" + "H1_PZ" * "H1_PZ") AS
magnitude FROM file
),
buckets AS (
  SELECT ROUND(magnitude / 10000, 0) * 10000 as bucket FROM m
)
SELECT bucket, COUNT(*)
FROM buckets
GROUP BY bucket
ORDER BY bucket;
*/

  TBranch *br_h1_px = nullptr;
  TBranch *br_h1_py = nullptr;
  TBranch *br_h1_pz = nullptr;

  double h1_px;
  double h1_py;
  double h1_pz;

  tree->SetBranchAddress("H1_PX", &h1_px, &br_h1_px);
  tree->SetBranchAddress("H1_PY", &h1_py, &br_h1_py);
  tree->SetBranchAddress("H1_PZ", &h1_pz, &br_h1_pz);

  auto nEntries = tree->GetEntries();
  std::chrono::steady_clock::time_point ts_first = std::chrono::steady_clock::now();
  /// group by and order by
  std::map<long, long> buckets;
  for (decltype(nEntries) entryId = 0; entryId < nEntries; ++entryId) {
    if ((entryId % 100000) == 0) {
        // if (measure) printf("processed %llu k events\n", entryId / 1000);
      // printf("dummy is %lf\n", dummy); abort();
    }

    tree->LoadTree(entryId);

    br_h1_px->GetEntry(entryId);
    br_h1_py->GetEntry(entryId);
    br_h1_pz->GetEntry(entryId);

    double magnitude = sqrt((h1_px * h1_px) + (h1_py * h1_py) + (h1_pz * h1_pz));
    long bucket = static_cast<long>(round(magnitude/10000))*10000;
    buckets[bucket] += 1;
  }
  if (show) {
      for (auto& [bucket, cnt] : buckets) {
          std::cout << bucket << "," <<  cnt << std::endl;
      }
  }
  std::cout << "found " << buckets.size() << " buckets with cnt[0] " << buckets[0] << std::endl;
  auto ts_end = std::chrono::steady_clock::now();
  auto runtime_init =
      std::chrono::duration_cast<std::chrono::microseconds>(ts_first - ts_init)
          .count();
  auto runtime_analyze =
      std::chrono::duration_cast<std::chrono::microseconds>(ts_end - ts_first)
          .count();

  std::cout << "Runtime-Initialization: " << runtime_init << "us" << std::endl;
  std::cout << "Runtime-Analysis: " << runtime_analyze << "us" << std::endl;

  if (measure)
    ps->Print();
}

auto main(int argc, char *argv[]) -> int {
    // open TTree at ../argv[1]
    if (argc < 2) {
        std::cerr << "No file provided" << std::endl;
        return 1;
    }
    auto file = TFile::Open(argv[1], "read");
    // print stats
    file->Print();
    // print TTree object 'DecayTree' stats
    auto tree = dynamic_cast<TTree *>(file->Get("DecayTree"));
    tree->Print();
    using clock = std::chrono::steady_clock;
    for (auto i = 0; i != 3; ++i) {
      auto start = clock::now();
      TreeQuery(tree, argc >= 3, argc >= 4);
      auto end = clock::now();
      std::cout << "run " << i << " total time: " << std::chrono::duration_cast<std::chrono::milliseconds>(end - start) .count() << "ms" << std::endl;
    }
    // execute equivalent of the following sql query:
    // close file
    file->Close();
    delete file;
}
